// src/main.rs
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod config;
mod validate; // ← direct in src/

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
        #[arg(value_name = "ZIP", required = true)]
        input: Vec<PathBuf>,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Validate FB2/XML files against XSD (streaming)
    Validate {
        #[arg(value_name = "FILE", required = true)]
        input: Vec<PathBuf>,

        /// Explicit XSD schema (overrides config.json)
        #[arg(short, long, value_name = "XSD")]
        xsd: Option<PathBuf>,

        #[arg(long)]
        strict: bool,
    },

    Group {/* TODO */},
    Index {/* TODO */},
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
            config::BookwealdConfig::create_default(*force)?;
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

        Commands::Validate {
            input,
            xsd,
            strict: _,
        } => {
            let xsd_ref = xsd.as_deref().and_then(|p| p.to_str());

            for file in input {
                if !file.exists() {
                    anyhow::bail!("File not found: {}", file.display());
                }

                tracing::info!("Validating {}", file.display());
                validate::streaming_validate(file, xsd_ref)
                    .with_context(|| format!("Failed to validate {}", file.display()))?;
            }

            println!("🎉 All files validated successfully!");
        }

        _ => println!("Command not implemented yet"),
    }

    Ok(())
}
