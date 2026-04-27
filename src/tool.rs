use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Dry-run. If set → no changes
    #[arg(short, long, value_name = "DRY_RUN")]
    dry_run: bool,

    /// Verbose output (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Initialize default configuration file
    Init {
        /// Overwrite existing config
        #[arg(long, short)]
        force: bool,
    },

    /// Extract FB2 books from ZIP archive
    Extract {
        /// Path(s) to ZIP file(s) containing ZIPs (one or more)
        #[arg(value_name = "INPUT", required = true)]
        input: Vec<PathBuf>,

        /// Output directory. If omitted → uses value from config library_dir
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

    /// Group books by author
    Group {
        #[arg(short, long, value_name = "DIR")]
        input: PathBuf,
        #[arg(short, long, value_name = "DIR", default_value_os_t = PathBuf::from("library"))]
        output: PathBuf,
    },

    /// Build book index
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

            let final_output = output.as_deref().unwrap_or(&config.library_dir);

            tracing::info!("Extracting {} input(s) → {:?}", input.len(), final_output);

            let dry_run = cli.dry_run || config.dry_run;
            bookweald_rs::extract::extract_zip_multi(&input, final_output, dry_run)
                .context("Failed to extract archive(s)")?;
        }

        Commands::Validate { input, strict } => {
            tracing::info!("Validating {:?} (strict: {})", input, strict);
            // TODO: bookweald_rs::validate_fb2...
        }
        Commands::Group { input, output } => {
            tracing::info!("Grouping books from {:?} into {:?}", input, output);
            // TODO: bookweald_rs::group_by_author...
        }
        Commands::Index { path, output } => {
            tracing::info!("Building index {:?} → {:?}", path, output);
            // TODO: bookweald_rs::build_index...
        }
    }

    Ok(())
}
