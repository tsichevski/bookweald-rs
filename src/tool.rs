use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

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
        /// Overwrite existing config
        #[arg(long, short)]
        force: bool,
    },

    /// Extract FB2 books from one or more ZIP files (flat, parallel)
    Extract {
        /// ZIP file(s) to extract (required positional arguments)
        #[arg(value_name = "ZIP", required = true)]
        input: Vec<PathBuf>,

        /// Output directory (defaults to value from config)
        #[arg(short, long, value_name = "DIR")]
        output: Option<PathBuf>,
    },

    /// Validate FB2 files
    Validate {
        #[arg(short, long, value_name = "PATH")]
        input: PathBuf,
        #[arg(long)]
        strict: bool,
    },

    /// Group books by author (TODO)
    Group {
        #[arg(short, long, value_name = "DIR")]
        input: PathBuf,
        #[arg(short, long, value_name = "DIR")]
        output: Option<PathBuf>,
    },

    /// Build book index (TODO)
    Index {
        #[arg(short, long, value_name = "DIR")]
        path: PathBuf,
        #[arg(short, long, value_name = "FILE", default_value_os_t = PathBuf::from("index.toml"))]
        output: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize tracing
    let level = match cli.verbose {
        0 => tracing::Level::INFO,
        1 => tracing::Level::DEBUG,
        _ => tracing::Level::TRACE,
    };
    tracing_subscriber::fmt().with_max_level(level).init();

    match &cli.command {
        Commands::Init { force } => {
            tracing::info!("Creating default configuration (force: {})", force);
            bookweald_rs::config::BookwealdConfig::create_default(*force)?;
        }

        Commands::Extract { input, output } => {
            let config = bookweald_rs::config::BookwealdConfig::load()?;

            let final_output = output.as_deref().unwrap_or(&config.target_dir);

            // CLI --jobs / -j overrides config, same for --dry-run
            let jobs = cli.jobs.unwrap_or(config.jobs);
            let effective_dry_run = cli.dry_run || config.dry_run;

            tracing::info!(
                "Extracting {} ZIP(s) using {} thread(s) (dry_run={})",
                input.len(),
                jobs,
                effective_dry_run
            );

            bookweald_rs::extract::extract_zip_multi(input, final_output, jobs, effective_dry_run)
                .context("Failed to extract archive(s)")?;
        }

        Commands::Validate { input, strict } => {
            tracing::info!("Validating {:?} (strict: {})", input, strict);
            // TODO
        }

        Commands::Group { input, output } => {
            tracing::info!("Grouping books from {:?} to {:?} (TODO)", input, output);
            // TODO — will respect global dry_run if needed later
        }

        Commands::Index { path, output } => {
            tracing::info!("Building index {:?} → {:?}", path, output);
            // TODO
        }
    }

    Ok(())
}
