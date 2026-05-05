//! Module responsible for extracting FB2 books from ZIP archives.
//!
//! This module provides high-performance, parallel extraction of `.fb2` files
//! using Rayon for multi-threading. It supports
//! dry-run mode, force-overwrite, and safe handling of ZIP entries.

use anyhow::{Context, Result};
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use tracing;

/// Extracts multiple ZIP archives in parallel, pulling out FB2 books.
///
/// # Arguments
///
/// * `inputs` - List of paths to ZIP files.
/// * `output` - Target directory where FB2 files will be placed.
/// * `dry_run` - If `true`, only logs what *would* happen without writing anything.
/// * `force` - If `true`, overwrites existing FB2 files in the output directory.
pub fn extract_zip_multi(
    inputs: &[PathBuf],
    output: &Path,
    dry_run: bool,
    force: bool,
) -> Result<()> {
    let results: Vec<_> = inputs
        .par_iter()
        .map(|zip_path| extract_single_zip(zip_path, output, dry_run, force))
        .collect();

    let mut all_entry_results = Vec::new();

    for outer in results {
        match outer {
            Ok(inner_vec) => all_entry_results.extend(inner_vec),
            Err(e) => return Err(e),
        }
    }

    let (successes, errors): (Vec<_>, Vec<_>) =
        all_entry_results.par_iter().partition(|r| r.is_ok());

    let num_success = successes.len();
    let num_errors = errors.len();

    tracing::info!(
        "Extraction completed: {} FB2 files found in {} inputs ({} succeeded, {} failed)",
        num_success + num_errors,
        inputs.len(),
        num_success,
        num_errors
    );

    if dry_run {
        tracing::info!("[dry-run] No files or directories were created");
    }

    Ok(())
}

/// Processes a single ZIP archive and extracts all contained `.fb2` files.
///
/// This function opens the ZIP once, then uses a parallel iterator with
/// `map_init` + unsafe re-opening (via metadata cloning) to allow concurrent
/// access to different entries without locking the whole archive.
///
/// # Returns
///
/// A `Vec<Result<()>>` — one result per FB2 file found.
fn extract_single_zip(
    zip_path: &Path,
    target_dir: &Path,
    dry_run: bool,
    force: bool,
) -> Result<Vec<Result<()>>> {
    let file = fs::File::open(zip_path)?;

    let archive = match zip::ZipArchive::new(file)
        .with_context(|| format!("Not a valid ZIP file: {}", zip_path.display()))
    {
        Ok(v) => v,
        Err(e) => {
            return Err(e);
        }
    };

    Ok((0..archive.len())
        .into_par_iter()
        .map_init(
            {
                let metadata = archive.metadata().clone();
                move || {
                    let file = fs::File::open(zip_path).unwrap();
                    unsafe { zip::ZipArchive::unsafe_new_with_metadata(file, metadata.clone()) }
                }
            },
            |archive, i| {
                let mut entry = archive.by_index(i).with_context(|| {
                    format!("Cannot read ZIP entry {i} in file: {}", zip_path.display())
                })?;

                let name = match entry.enclosed_name() {
                    Some(n) => n.to_owned(),
                    None => return Ok(()),
                };

                let basename = name.file_name().unwrap().to_string_lossy().to_string();
                if !basename.to_lowercase().ends_with(".fb2") {
                    return Ok(());
                }

                let out_path = target_dir.join(&basename);

                if out_path.exists() && !force {
                    tracing::debug!("Skipping existing (use --force to overwrite): {}", basename);
                    return Ok(());
                }

                if dry_run {
                    tracing::debug!("[dry-run] Would write: {}", basename);
                    return Ok(());
                }

                // Do real changes in fs
                if let Some(parent) = out_path.parent() {
                    fs::create_dir_all(parent).with_context(|| {
                        format!("Cannot create directory: {}", parent.display())
                    })?;
                }

                let mut out_file = fs::File::create(&out_path).with_context(|| {
                    format!("Cannot read ZIP entry {i} in file: {}", zip_path.display())
                })?;

                std::io::copy(&mut entry, &mut out_file).with_context(|| {
                    format!("Cannot copy ZIP entry {i} to file: {}", out_path.display())
                })?;

                tracing::debug!("✅ Extracted: {}", basename);
                Ok(())
            },
        )
        .collect())
}
