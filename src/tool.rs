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
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Initialize default configuration file (like OCaml `init`)
    Init {
        /// Overwrite existing config
        #[arg(long, short)]
        force: bool,
    },

    /// Extract FB2 books from ZIP archive (like OCaml `export`)
    Extract {
        /// Path to ZIP file or directory containing ZIPs (positional, required)
        #[arg(value_name = "INPUT", required = true)]
        input: PathBuf,

        /// Output directory
        #[arg(short, long, value_name = "DIR", default_value_os_t = PathBuf::from("library"))]
        output: PathBuf,
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
            tracing::info!("Extracting from {:?} → {:?}", input, output);
            bookweald_rs::extract::extract_zip(input, output)
                .context("Failed to extract archive")?;
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
