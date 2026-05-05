use anyhow::Result;
use bookweald_rs::blacklist;
use bookweald_rs::db;
use clap::{Parser, Subcommand};
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::{
    path::{Path, PathBuf},
    usize,
};

use bookweald_rs::alias;
use bookweald_rs::config::BookwealdConfig;
use bookweald_rs::index;
use bookweald_rs::validate;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Args {
    #[command(subcommand)]
    command: Commands,

    /// Verbose output (-v, -vv, -vvv)
    #[arg(long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,

    /// Config path location (overrides default ~/.config/bookweald/config.json)
    #[arg(short, long, value_name = "PATH", global = true)]
    config: Option<PathBuf>,

    /// Maximum number of CPU threads Bookweald may use for heavy computation
    ///
    /// Lower this value if you want the machine to stay responsive while building large libraries.
    #[arg(short = 'j', long = "jobs", value_name = "JOBS", global = true)]
    jobs: Option<usize>,

    /// Number of async I/O threads
    #[arg(short, long, value_name = "N", global = true)]
    pub io_threads: Option<usize>,

    /// Maximum threads for blocking/offload operations (spawn_blocking pool)
    ///
    /// Rarely needs tuning. Controls background file I/O and short CPU bursts.
    /// Default: same as jobs.
    #[arg(short, long, value_name = "N", global = true)]
    pub blocking_threads: Option<usize>,

    /// Do not actually do any changes
    #[arg(short, long, short = 'n', global = true)]
    dry_run: bool,
}

fn build_runtime(args: &Args) -> tokio::runtime::Runtime {
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(8);

    let jobs = args.jobs.unwrap_or((cores * 3) / 4); // ~75% for CPU
    let io = args.io_threads.unwrap_or((cores / 2).max(4));
    let blocking = args.blocking_threads.unwrap_or(jobs.max(8));

    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(io)
        .max_blocking_threads(blocking)
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime")
}

fn init_rayon(args: &Args) {
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(8);
    let jobs = args.jobs.unwrap_or(cores);

    rayon::ThreadPoolBuilder::new()
        .num_threads(jobs)
        .thread_name(|i| format!("bookweald-{}", i))
        .build_global()
        .expect("Failed to initialize Rayon global pool");
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

    /// Drop DB contents and initialize DB schema
    SchemaInit {
        /// Overwrite existing schema
        #[arg(short, long)]
        overwrite: bool,
    },

    /// TODO Group books by author (create author sub-directories)
    Group {/* TODO */},

    /// TODO Parse all FB2 files in the specified directory and add them to index
    Index {
        /// Paths to files or directories to index
        #[arg(value_name = "PATH", required = true, num_args(1..))]
        input: Vec<PathBuf>,

        /// Overwrite existing books
        #[arg(short, long)]
        overwrite: bool,
    },
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
    let args = Args::parse();

    tracing_config::init!();

    let runtime = build_runtime(&args); // we already have the runtime from #[tokio::main], but you can also use rt.block_on(...)
    init_rayon(&args);

    match &args.command {
        Commands::Init { force } => {
            tracing::info!("Creating default configuration (force: {})", force);
            BookwealdConfig::create_default(args.config, *force)
        }

        Commands::Extract {
            input,
            output,
            force,
        } => {
            let config = bookweald_rs::config::BookwealdConfig::load(args.config)?;
            let final_output = output.as_deref().unwrap_or(&config.library_dir);
            let effective_dry_run = args.dry_run || config.dry_run;

            tracing::info!(
                "Extracting {} ZIP(s) (dry_run={}, force={})",
                input.len(),
                effective_dry_run,
                force
            );

            runtime.block_on(async {
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
            let config = bookweald_rs::config::BookwealdConfig::load(args.config)?;
            let effective_dry_run = args.dry_run || config.dry_run;
            let xsd_ref = xsd.as_deref().and_then(|p| p.to_str());

            tracing::info!(
                "Validating {} locations (dry_run={})",
                input.len(),
                effective_dry_run,
            );

            let mut files: Vec<PathBuf> = Vec::new();
            for path in input {
                files.extend(collect_fb2_files(path)?);
            }
            let total = files.len();
            let blacklist = &config.blacklist;
            let blacklisted = blacklist::blacklisted(blacklist)?;
            let (black, not_black): (Vec<_>, Vec<_>) =
                files.into_iter().partition(|p| blacklisted(p) ^ *reverse);
            if !black.is_empty() {
                tracing::info!("{} files, {} blacklisted", total, black.len());
            }

            runtime.block_on(async {
                let results: Vec<_> = validate::validate(&not_black, xsd_ref);

                if let Some(file) = blacklist {
                    if let Some(parent) = file.parent() {
                        fs::create_dir_all(parent)?;
                    }

                    let mut ch: File = OpenOptions::new()
                        .append(true)
                        .create(true)
                        .open(file)
                        .unwrap();

                    for (file, result) in not_black.iter().zip(&results) {
                        if let Err(e) = result {
                            let basename = file
                                .file_prefix()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string();
                            writeln!(ch, "{}|{}", basename, e)?;
                        }
                    }
                }

                let (successes, errors): (Vec<_>, Vec<_>) =
                    results.into_iter().partition(Result::is_ok);

                tracing::info!(
                    "Validation completed: books processed: {} ({} OK, {} failed)",
                    not_black.len(),
                    successes.len(),
                    errors.len()
                );

                if effective_dry_run {
                    tracing::info!("[dry-run] Blacklist was not modified");
                }
                Ok(())
            })
        }

        Commands::SchemaInit { overwrite } => {
            let config = bookweald_rs::config::BookwealdConfig::load(args.config)?;
            let dry_run = args.dry_run || config.dry_run;
            let cd = config.database;
            tracing::info!("Initialize DB schema");
            runtime.block_on(async {
                let admin_pool = match db::connect_async(
                    &cd.host,
                    cd.port,
                    &cd.admin,
                    &cd.admin_passwd,
                    &cd.name,
                )
                .await
                {
                    Ok(p) => p,
                    Err(e) => {
                        anyhow::bail!(format!("Failed to create admin database pool: {}", e))
                    }
                };

                if dry_run {
                    Ok(())
                } else {
                    match db::init_schema(&admin_pool, *overwrite).await {
                        Ok(()) => Ok(()),
                        Err(e) => {
                            anyhow::bail!(format!("Failed to initialize database schema: {}", e))
                        }
                    }
                }
            })
        }
        Commands::Index { input, overwrite } => {
            let config = bookweald_rs::config::BookwealdConfig::load(args.config)?;
            let effective_dry_run = args.dry_run || config.dry_run;

            tracing::info!(
                "Indexing {} location(s) (dry_run={})",
                input.len(),
                effective_dry_run,
            );

            let mut files: Vec<PathBuf> = Vec::new();
            for path in input {
                files.extend(collect_fb2_files(path)?);
            }
            let total = files.len();
            let blacklist = &config.blacklist;
            let blacklisted = blacklist::blacklisted(blacklist)?;
            let (black, not_black): (Vec<_>, Vec<_>) =
                files.into_iter().partition(|p| blacklisted(p));
            if !black.is_empty() {
                tracing::info!("{} files, {} blacklisted", total, black.len());
            }

            let aliases = match config.alias_file {
                None => None,
                Some(path) => Some(alias::load_aliases(&path)?),
            };

            let cd = config.database;

            runtime.block_on(async {
                let pool =
                    db::connect_async(&cd.host, cd.port, &cd.user, &cd.passwd, &cd.name).await?;
                index::index(&not_black, &aliases, *overwrite, &pool).await?;

                tracing::info!("Indexing completed: books processed: {}", not_black.len(),);

                if effective_dry_run {
                    tracing::info!("[dry-run] Db was not modified");
                }
                Ok(())
            })
        }

        _ => anyhow::bail!("Command {:?} is not implemented yet", &args.command),
    }
}
