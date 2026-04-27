use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use tracing;

/// Statistics for the whole extraction run
#[derive(Default)]
struct ExtractStats {
    processed_archives: AtomicUsize,
    extracted_fb2: AtomicUsize,
    skipped_existing: AtomicUsize,
}

/// Extract from one or more inputs (files or directories) — pure function
pub fn extract_zip_multi(inputs: &[PathBuf], output: &Path, dry_run: bool) -> Result<()> {
    fs::create_dir_all(output).context("Failed to create output directory")?;

    let stats = ExtractStats::default();

    if dry_run {
        tracing::info!("**Dry-run**: no files will change");
    }

    for input in inputs {
        tracing::info!("Processing input: {}", input.display());

        if input.is_file() && input.extension().map_or(false, |e| e == "zip") {
            extract_single_zip(input, output, &stats, dry_run)?;
        } else if input.is_dir() {
            visit_dirs(input, &mut |zip_path| {
                tracing::debug!("Found archive: {}", zip_path.display());
                let _ = extract_single_zip(zip_path, output, &stats, dry_run);
            })?;
        } else {
            tracing::warn!("Skipping invalid input: {}", input.display());
        }
    }

    // Final statistics (OCaml-style summary)
    let processed = stats.processed_archives.load(Ordering::Relaxed);
    let extracted = stats.extracted_fb2.load(Ordering::Relaxed);
    let skipped = stats.skipped_existing.load(Ordering::Relaxed);

    tracing::info!("Archives processed : {}", processed);
    tracing::info!("FB2 files extracted: {}", extracted);
    tracing::info!("Skipped (existing) : {}", skipped);

    Ok(())
}

// Pure std recursive visitor (unchanged)
fn visit_dirs(dir: &Path, cb: &mut dyn FnMut(&Path)) -> Result<()> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                visit_dirs(&path, cb)?;
            } else if path.extension().map_or(false, |e| e == "zip") {
                cb(&path);
            }
        }
    }
    Ok(())
}

fn extract_single_zip(
    zip_path: &Path,
    target_dir: &Path,
    stats: &ExtractStats,
    dry_run: bool,
) -> Result<()> {
    stats.processed_archives.fetch_add(1, Ordering::Relaxed);

    let file = fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = match file.enclosed_name() {
            Some(n) => n.to_owned(),
            None => continue,
        };

        let basename = name.file_name().unwrap().to_string_lossy().to_string();
        if !basename.to_lowercase().ends_with(".fb2") {
            continue;
        }

        let out_path = target_dir.join(&basename);

        if out_path.exists() {
            stats.skipped_existing.fetch_add(1, Ordering::Relaxed);
            tracing::debug!("Skipping existing: {}", out_path.display());
            continue;
        }

        if dry_run {
            tracing::debug!("✅ Would extract: {}", out_path.display());
        } else {
            fs::create_dir_all(target_dir)?;
            let mut out_file = fs::File::create(&out_path)?;
            std::io::copy(&mut file, &mut out_file)?;
            tracing::debug!("✅ Extracted: {}", out_path.display());
        }

        stats.extracted_fb2.fetch_add(1, Ordering::Relaxed);
    }

    Ok(())
}
