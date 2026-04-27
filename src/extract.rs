use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use tracing;

use crate::fb2_parse::parse_book_info;

/// Extract FB2 books from ZIP(s) to target_dir.
/// Supports: single .zip, or directory (recursive, pure std).
pub fn extract_zip(input: &Path, output: &Path, group: bool) -> Result<()> {
    fs::create_dir_all(output).context("Failed to create output directory")?;

    tracing::info!(
        "Extracting from {:?} → {:?} (group={})",
        input,
        output,
        group
    );

    if input.is_file() && input.extension().map_or(false, |e| e == "zip") {
        extract_single_zip(input, output, group)?;
    } else if input.is_dir() {
        visit_dirs(input, &mut |zip_path| {
            tracing::debug!("Processing archive: {}", zip_path.display());
            let _ = extract_single_zip(zip_path, output, group); // continue on errors
        })?;
    } else {
        anyhow::bail!("Input must be a .zip file or a directory containing ZIPs");
    }

    Ok(())
}

/// Pure std recursive directory visitor (no walkdir)
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

fn extract_single_zip(zip_path: &Path, target_dir: &Path, group: bool) -> Result<()> {
    let file = fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = match file.enclosed_name() {
            Some(n) => n.to_owned(),
            None => continue,
        };

        if !name.to_string_lossy().to_lowercase().ends_with(".fb2") {
            continue;
        }

        let basename = name.file_name().unwrap().to_string_lossy().to_string();
        let mut out_path = target_dir.join(&basename);

        // Author grouping (metadata parse)
        if group {
            match parse_book_info(zip_path, None) {
                Ok(book) => {
                    if let Some(author) = book.authors.first() {
                        let author_dir = sanitize_filename(&author.id);
                        let author_path = target_dir.join(author_dir);
                        fs::create_dir_all(&author_path)?;
                        out_path = author_path.join(&basename);
                    }
                }
                Err(e) => tracing::warn!("Failed to parse {} for grouping: {}", basename, e),
            }
        }

        if out_path.exists() {
            tracing::info!("Skipping existing: {}", out_path.display());
            continue;
        }

        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut out_file = fs::File::create(&out_path)?;
        std::io::copy(&mut file, &mut out_file)?;

        tracing::info!("✅ Extracted: {}", out_path.display());
    }

    Ok(())
}

fn sanitize_filename(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            c if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' => c,
            _ => '_',
        })
        .collect::<String>()
        .trim()
        .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_")
        .replace("  ", " ")
}
