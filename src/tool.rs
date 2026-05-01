use anyhow::Result;
use bookweald_rs::blacklist;
use clap::{Parser, Subcommand};
use std::{
    path::{Path, PathBuf},
    usize,
};

use bookweald_rs::config::BookwealdConfig;
use bookweald_rs::validate;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Verbose output (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,

    /// Config path location (overrides default ~/.config/bookweald/config.json)
    #[arg(short, long, value_name = "PATH", global = true)]
    config: Option<PathBuf>,

    /// Number of parallel jobs (overrides config.jobs)
    #[arg(short = 'j', long = "jobs", value_name = "N", global = true)]
    jobs: Option<usize>,

    /// Do not actually write any files or directories (global)
    #[arg(long, short = 'n', global = true)]
    dry_run: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Initialize default configuration file
    Init {
        #[arg(long, short)]
        force: bool,
    },

    /// Extract FB2 books from ZIP files
    Extract {
        /// Input ZIP file(s)
        #[arg(value_name = "ZIP", required = true, num_args(1..))]
        input: Vec<PathBuf>,

        /// Explicitly set the output directory (overrides config.library_dir)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Force existing files overwrite
        #[arg(short, long)]
        force: bool,
    },

    /// Validate FB2/XML files against XSD (streaming)
    Validate {
        /// Paths to files or directories to validate
        #[arg(value_name = "PATH", required = true, num_args(1..))]
        input: Vec<PathBuf>,

        /// Optional XSD schema, if missing, only base XML structure conformance will be validate
        #[arg(short, long, value_name = "XSD")]
        xsd: Option<PathBuf>,

        /// Reverse black list: process blacklisted files only.
        #[arg(short, long)]
        reverse: bool,
    },

    Group {/* TODO */},
    Index {/* TODO */},
}

/// Recursively scans the given list of paths (files or directories)
/// and collects all files with extensions `.fb2` or `.fb2.zip` (case-insensitive).
///
/// Returns a `Vec<PathBuf>` of matching file paths.
pub fn collect_fb2_files(path: &PathBuf) -> Result<Vec<PathBuf>> {
    if !path.exists() {
        anyhow::bail!("Path does not exist: {}", path.display());
    }

    fn is_fb2_file(path: &Path) -> bool {
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_lowercase());

        matches!(ext.as_deref(), Some("fb2") | Some("fb2.zip"))
    }
    let mut fb2_files = Vec::new();
    if path.is_file() {
        if is_fb2_file(path) {
            fb2_files.push(path.clone());
        }
    } else {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            fb2_files.extend(collect_fb2_files(&path)?);
        }
    }

    Ok(fb2_files)
}

fn run_parallel<OP, R>(jobs: usize, op: OP) -> R
where
    OP: FnOnce() -> R + Send,
    R: Send,
{
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(jobs)
        .build()
        .unwrap();

    pool.install(op)
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    tracing_config::init!();

    match &cli.command {
        Commands::Init { force } => {
            tracing::info!("Creating default configuration (force: {})", force);
            BookwealdConfig::create_default(cli.config, *force)?;
        }

        Commands::Extract {
            input,
            output,
            force,
        } => {
            let config = bookweald_rs::config::BookwealdConfig::load(cli.config)?;
            let final_output = output.as_deref().unwrap_or(&config.library_dir);
            let jobs = cli.jobs.unwrap_or(config.jobs);
            let effective_dry_run = cli.dry_run || config.dry_run;

            tracing::info!(
                "Extracting {} ZIP(s) using {} thread(s) (dry_run={}, force={})",
                input.len(),
                jobs,
                effective_dry_run,
                force
            );
            run_parallel(jobs, || {
                bookweald_rs::extract::extract_zip_multi(
                    input,
                    final_output,
                    effective_dry_run,
                    *force,
                )
            })
        }

        Commands::Validate {
            input,
            xsd,
            reverse,
        } => {
            let config = bookweald_rs::config::BookwealdConfig::load(cli.config)?;
            let jobs = cli.jobs.unwrap_or(config.jobs);
            let effective_dry_run = cli.dry_run || config.dry_run;
            let xsd_ref = xsd.as_deref().and_then(|p| p.to_str());

            tracing::info!(
                "Validating {} locations using {} thread(s) (dry_run={})",
                input.len(),
                jobs,
                effective_dry_run,
            );

            let mut files: Vec<PathBuf> = Vec::new();
            for path in input {
                files.extend(collect_fb2_files(path)?);
            }
            let total = files.len();
            let blacklisted = blacklist::blacklisted(&config.blacklist)?;
            let (black, not_black): (Vec<_>, Vec<_>) =
                files.into_iter().partition(|p| blacklisted(p) ^ *reverse);
            run_parallel(jobs, || {
                let results: Vec<_> = validate::validate(&not_black, xsd_ref);

                for (file, result) in not_black.iter().zip(&results) {
                    if let Err(e) = result {
                        let basename = file.file_prefix().unwrap_or_default().to_string_lossy();
                        println!("{}|{}", basename, e);
                    }
                }

                let (successes, errors): (Vec<_>, Vec<_>) =
                    results.into_iter().partition(Result::is_ok);

                tracing::info!(
                    "Validation completed books found {}, blacklisted: {}, processed {} ({} OK, {} failed)",
                    total,
                    black.len(),
                    not_black.len(),
                    successes.len(),
                    errors.len()
                );

                if effective_dry_run {
                    tracing::info!("[dry-run] Blacklist was not modified");
                }
            });
        }
        _ => println!("Command not implemented yet"),
    }

    Ok(())
}
