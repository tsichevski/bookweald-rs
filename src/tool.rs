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

    /// Extract FB2 books from ZIP archive
    Extract {
        #[arg(short, long, value_name = "PATH")]
        input: PathBuf,

        #[arg(short, long, value_name = "DIR", default_value_os_t = PathBuf::from("library"))]
        output: PathBuf,

        /// Also create author subdirectories (Author/Lastname/)
        #[arg(short, long)]
        group: bool,
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
            return Ok(());
        }

        Commands::Extract {
            input,
            output,
            group,
        } => {
            tracing::info!("Extracting from {:?} → {:?}", input, output);
            if *group {
                tracing::info!("Author grouping enabled");
            }
            bookweald_rs::extract::extract_zip(input, output, *group)
                .context("Failed to extract archive")?;
        }

        Commands::Validate { input, strict } => {
            tracing::info!("Validating {:?} (strict: {})", input, strict);
            // bookweald_rs::validate_fb2...
        }

        Commands::Group { input, output } => {
            tracing::info!("Grouping books from {:?} into {:?}", input, output);
            // bookweald_rs::group_by_author...
        }

        Commands::Index { path, output } => {
            tracing::info!("Building index {:?} → {:?}", path, output);
            // bookweald_rs::build_index...
        }
    }

    Ok(())
}
