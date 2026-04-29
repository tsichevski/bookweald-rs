use anyhow::{Context, Result};
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use tracing;

/// Parallel extraction: one ZIP archive per thread (best balance for typical FB2 collections)
pub fn extract_zip_multi(
    inputs: &[PathBuf],
    output: &Path,
    num_threads: usize,
    dry_run: bool,
    force: bool,
) -> Result<()> {
    tracing::info!(
        "Extracting {} ZIP(s) using {} thread(s) (dry_run={}, force={})",
        inputs.len(),
        num_threads,
        dry_run,
        force
    );

    if !dry_run {
        fs::create_dir_all(output).context("Failed to create output directory")?;
    }

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build()
        .unwrap();

    pool.install(|| {
        let (successes, errors): (Vec<_>, Vec<_>) = inputs
            .par_iter()
            .flat_map(|zip_path| extract_single_zip(zip_path, output, dry_run, force))
            .partition(Result::is_ok);

        let num_success = successes.len();
        let num_errors = errors.len();

        tracing::info!(
            "Extraction completed: {} FB2 files found in {} inputs ({} succeeded, {} failed)",
            num_success + num_errors,
            inputs.len(),
            num_success,
            num_errors
        );
    });

    if dry_run {
        tracing::info!("[dry-run] No files or directories were created");
    }

    Ok(())
}

fn extract_single_zip(
    zip_path: &Path,
    target_dir: &Path,
    dry_run: bool,
    force: bool,
) -> Vec<Result<()>> {
    let mut result: Vec<Result<()>> = Vec::new();
    let file = match fs::File::open(zip_path) {
        Ok(file) => file,
        Err(e) => {
            result.push(Err(e.into()));
            return result;
        }
    };

    let mut archive = match zip::ZipArchive::new(file)
        .with_context(|| format!("Not a valid ZIP file: {}", zip_path.display()))
    {
        Ok(v) => v,
        Err(e) => {
            result.push(Err(e));
            return result;
        }
    };

    for i in 0..archive.len() {
        let mut entry = match archive
            .by_index(i)
            .with_context(|| format!("Cannot read ZIP entry {i} in file: {}", zip_path.display()))
        {
            Ok(v) => v,
            Err(e) => {
                result.push(Err(e));
                return result;
            }
        };
        let name = match entry.enclosed_name() {
            Some(n) => n.to_owned(),
            None => continue,
        };

        let basename = name.file_name().unwrap().to_string_lossy().to_string();
        if !basename.to_lowercase().ends_with(".fb2") {
            continue;
        }

        let out_path = target_dir.join(&basename);

        if out_path.exists() && !force {
            tracing::debug!("Skipping existing (use --force to overwrite): {}", basename);
            continue;
        }

        if dry_run {
            tracing::debug!("[dry-run] Would extract: {}", basename);
            continue;
        }

        // Real extraction
        if let Some(parent) = out_path.parent() {
            match fs::create_dir_all(parent)
                .with_context(|| format!("Cannot create directory: {}", parent.display()))
            {
                Ok(v) => v,
                Err(e) => {
                    result.push(Err(e));
                    continue;
                }
            }
        }

        let mut out_file = match fs::File::create(&out_path)
            .with_context(|| format!("Cannot read ZIP entry {i} in file: {}", zip_path.display()))
        {
            Ok(v) => v,
            Err(e) => {
                result.push(Err(e));
                continue;
            }
        };

        match std::io::copy(&mut entry, &mut out_file)
            .with_context(|| format!("Cannot copy ZIP entry {i} to file: {}", out_path.display()))
        {
            Ok(_) => result.push(Ok(())),
            Err(e) => {
                result.push(Err(e));
                continue;
            }
        };

        tracing::debug!("✅ Extracted: {}", basename);
    }

    result
}
