use crate::config::BookwealdConfig;
use fastxml::schema::{StreamValidator, parse_xsd};
use std::fs::File;
use std::io::{BufReader, Cursor, Read};
use std::path::Path;
use std::sync::Arc;

pub fn streaming_validate(
    path: &Path,
    explicit_xsd: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let filename = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| format!("Invalid filename: {}", path.display()))?;

    println!("🔍 Streaming XSD validation with fastxml: {}", filename);

    let is_zip = path.extension().and_then(|e| e.to_str()) == Some("zip");

    // ── Prepare reader (ZIP requires special handling for lifetime) ──
    let raw_reader: Box<dyn Read> = if is_zip {
        println!("   (decompressing .fb2.zip into memory for validation)");
        let zip_file = File::open(path)?;
        let mut archive = zip::ZipArchive::new(zip_file)?;
        let mut entry = archive.by_index(0)?;

        let mut content = Vec::new();
        entry.read_to_end(&mut content)?;

        Box::new(Cursor::new(content))
    } else {
        // Normal .fb2 → true streaming, no full load
        Box::new(File::open(path)?)
    };

    let buf_reader = BufReader::new(raw_reader);

    // ── Determine which schema to use ──
    let errors = if let Some(xsd) = explicit_xsd {
        println!("   Using explicit schema: {}", xsd);
        let xsd_bytes = std::fs::read(xsd)?;
        let schema = Arc::new(parse_xsd(&xsd_bytes)?);

        StreamValidator::new(schema)
            .with_max_errors(200)
            .validate(buf_reader)?
    } else {
        println!("   Looking up schema by namespace from config.json...");
        let config = BookwealdConfig::load()?;

        let fb2_ns = "http://www.gribuser.ru/xml/fictionbook/2.0";

        if let Some(schema_path) = config.get_schema_for_namespace(fb2_ns) {
            println!(
                "   Using mapped schema for FictionBook 2.0: {}",
                schema_path
            );
            let xsd_bytes = std::fs::read(&schema_path)?;
            let schema = Arc::new(parse_xsd(&xsd_bytes)?);

            StreamValidator::new(schema)
                .with_max_errors(200)
                .validate(buf_reader)?
        } else {
            return Err(
                "No schema mapping found in config.json for this document.\n\
                 Please add it under \"namespaces\" or use --xsd flag."
                    .into(),
            );
        }
    };

    // ── Report result ──
    if errors.is_empty() {
        println!("✅ {} is valid according to XSD (streaming)", filename);
        Ok(())
    } else {
        println!(
            "❌ Validation failed for {} ({} errors):",
            filename,
            errors.len()
        );
        for err in errors.iter().take(15) {
            let level = match err.level {
                fastxml::ErrorLevel::Warning => "WARN",
                fastxml::ErrorLevel::Error => "ERROR",
                fastxml::ErrorLevel::Fatal => "FATAL",
            };

            let line = err
                .line()
                .map(|l| l.to_string())
                .unwrap_or_else(|| "?".to_string());

            println!("   [{}] line {}: {}", level, line, err.message);
        }
        if errors.len() > 15 {
            println!("   ... and {} more errors", errors.len() - 15);
        }
        Err("XSD validation failed".into())
    }
}
