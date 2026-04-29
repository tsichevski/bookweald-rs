use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};

use bookweald_rs::config::BookwealdConfig;
mod validate;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Verbose output (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,

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
        /// Explicit path to file(s) to validate, if not set, the config library_dir will be used
        #[arg(value_name = "PATH")]
        input: Vec<PathBuf>,

        /// Explicit XSD schema (overrides config.json)
        #[arg(short, long, value_name = "XSD")]
        xsd: Option<PathBuf>,
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

fn main() -> Result<()> {
    let cli = Cli::parse();

    let level = match cli.verbose {
        0 => tracing::Level::INFO,
        1 => tracing::Level::DEBUG,
        _ => tracing::Level::TRACE,
    };
    tracing_subscriber::fmt().with_max_level(level).init();

    match &cli.command {
        Commands::Init { force } => {
            tracing::info!("Creating default configuration (force: {})", force);
            BookwealdConfig::create_default(*force)?;
        }

        Commands::Extract {
            input,
            output,
            force,
        } => {
            let config = bookweald_rs::config::BookwealdConfig::load()?;

            let final_output = output.as_deref().unwrap_or(&config.library_dir);

            // CLI --jobs / -j overrides config, same for --dry-run
            let jobs = cli.jobs.unwrap_or(config.jobs);
            let effective_dry_run = cli.dry_run || config.dry_run;

            bookweald_rs::extract::extract_zip_multi(
                input,
                final_output,
                jobs,
                effective_dry_run,
                *force,
            )
            .context("Failed to extract archive(s)")?;
        }

        Commands::Validate { input, xsd } => {
            let xsd_ref = xsd.as_deref().and_then(|p| p.to_str());
            let mut files: Vec<PathBuf> = Vec::new();
            for path in input {
                files.extend(collect_fb2_files(path)?);
            }
            for path in &files {
                validate::validate(&path, xsd_ref)
                    .with_context(|| format!("Failed to validate {}", path.display()))?
            }
            println!("🎉 All files validated successfully!");
        }
        _ => println!("Command not implemented yet"),
    }

    Ok(())
}
