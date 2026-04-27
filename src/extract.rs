use anyhow::{Context, Result};
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use tracing;

#[derive(Default)]
struct ExtractStats {
    processed_archives: AtomicUsize,
    extracted_fb2: AtomicUsize,
    skipped_existing: AtomicUsize,
}

/// Parallel extraction: one ZIP archive per thread (best balance for typical FB2 collections)
pub fn extract_zip_multi(
    inputs: &[PathBuf],
    output: &Path,
    jobs: usize,
    dry_run: bool,
) -> Result<()> {
    tracing::info!(
        "Extracting {} ZIP file(s) → {:?} with {} threads (dry_run={})",
        inputs.len(),
        output,
        jobs,
        dry_run
    );

    let stats = ExtractStats::default();

    if !dry_run {
        fs::create_dir_all(output).context("Failed to create output directory")?;
    }

    rayon::ThreadPoolBuilder::new()
        .num_threads(jobs)
        .build()?
        .install(|| {
            inputs.par_iter().for_each(|zip_path| {
                if let Err(e) = extract_single_zip(zip_path, output, &stats, dry_run) {
                    tracing::error!("Failed to process {}: {}", zip_path.display(), e);
                }
            });
        });

    let processed = stats.processed_archives.load(Ordering::Relaxed);
    let extracted = stats.extracted_fb2.load(Ordering::Relaxed);
    let skipped = stats.skipped_existing.load(Ordering::Relaxed);

    tracing::info!("=== Extraction completed ===");
    tracing::info!("Archives processed : {}", processed);
    tracing::info!("FB2 files extracted: {}", extracted);
    tracing::info!("Skipped (existing) : {}", skipped);
    if dry_run {
        tracing::info!("[dry-run] No files or directories were created");
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

    let file =
        fs::File::open(zip_path).with_context(|| format!("Cannot open {}", zip_path.display()))?;

    let mut archive = zip::ZipArchive::new(file)
        .with_context(|| format!("Not a valid ZIP file: {}", zip_path.display()))?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = match entry.enclosed_name() {
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
            tracing::debug!("Skipping existing: {}", basename);
            continue;
        }

        stats.extracted_fb2.fetch_add(1, Ordering::Relaxed);

        if dry_run {
            tracing::debug!("[dry-run] Would extract: {}", basename);
            continue;
        }

        // Real extraction
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut out_file = fs::File::create(&out_path)?;
        std::io::copy(&mut entry, &mut out_file)?;

        tracing::debug!("✅ Extracted: {}", basename);
    }

    Ok(())
}
