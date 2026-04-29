use anyhow::{Context, Result};
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use tracing;

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

    let archive = match zip::ZipArchive::new(file)
        .with_context(|| format!("Not a valid ZIP file: {}", zip_path.display()))
    {
        Ok(v) => v,
        Err(e) => {
            result.push(Err(e));
            return result;
        }
    };

    (0..archive.len())
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
                let mut entry = match archive.by_index(i).with_context(|| {
                    format!("Cannot read ZIP entry {i} in file: {}", zip_path.display())
                }) {
                    Ok(v) => v,
                    Err(e) => {
                        return Err(e);
                    }
                };
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
                    tracing::debug!("[dry-run] Would extract: {}", basename);
                    return Ok(());
                }

                // Real extraction
                if let Some(parent) = out_path.parent() {
                    match fs::create_dir_all(parent)
                        .with_context(|| format!("Cannot create directory: {}", parent.display()))
                    {
                        Ok(v) => v,
                        Err(e) => return Err(e),
                    }
                }

                let mut out_file = match fs::File::create(&out_path).with_context(|| {
                    format!("Cannot read ZIP entry {i} in file: {}", zip_path.display())
                }) {
                    Ok(v) => v,
                    Err(e) => return Err(e),
                };

                match std::io::copy(&mut entry, &mut out_file).with_context(|| {
                    format!("Cannot copy ZIP entry {i} to file: {}", out_path.display())
                }) {
                    Ok(_) => {
                        tracing::debug!("✅ Extracted: {}", basename);
                        Ok(())
                    }
                    Err(e) => Err(e),
                }
            },
        )
        .collect()
}
