// src/validate.rs
use crate::config::BookwealdConfig;
use anyhow::{Context, Result};
use fastxml::schema::{StreamValidator, parse_xsd};
use std::fs::File;
use std::io::{BufReader, Cursor, Read};
use std::path::Path;
use std::sync::Arc;

pub fn streaming_validate(path: &Path, explicit_xsd: Option<&str>) -> Result<()> {
    let filename = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow::anyhow!("Invalid filename: {}", path.display()))?;

    println!("🔍 Validating: {}", filename);

    let is_zip = path.extension().and_then(|e| e.to_str()) == Some("zip");

    let raw_reader: Box<dyn Read> = if is_zip {
        println!("   (ZIP → memory decompress)");
        let zip_file = File::open(path)?;
        let mut archive = zip::ZipArchive::new(zip_file)?;
        let mut entry = archive.by_index(0)?;

        let mut content = Vec::new();
        entry.read_to_end(&mut content)?;
        Box::new(Cursor::new(content))
    } else {
        Box::new(File::open(path)?)
    };

    let buf_reader = BufReader::new(raw_reader);

    let errors = if let Some(xsd) = explicit_xsd {
        println!("   Using explicit schema: {}", xsd);
        let xsd_bytes = std::fs::read(xsd).with_context(|| format!("Cannot read XSD {}", xsd))?;
        let schema = Arc::new(parse_xsd(&xsd_bytes)?);

        StreamValidator::new(schema)
            .with_max_errors(500)
            .validate(buf_reader)?
    } else {
        println!("   Using namespace mapping from config.json...");
        let config = BookwealdConfig::load()?;
        let fb2_ns = "http://www.gribuser.ru/xml/fictionbook/2.0";

        if let Some(schema_path) = config.get_schema_for_namespace(fb2_ns) {
            println!("   Schema: {}", schema_path);
            let xsd_bytes = std::fs::read(&schema_path)
                .with_context(|| format!("Cannot read schema {}", schema_path))?;
            let schema = Arc::new(parse_xsd(&xsd_bytes)?);

            StreamValidator::new(schema)
                .with_max_errors(500)
                .validate(buf_reader)?
        } else {
            anyhow::bail!("No schema mapping in config.json for FictionBook. Use --xsd");
        }
    };

    if errors.is_empty() {
        println!("✅ {} is valid", filename);
        Ok(())
    } else {
        println!(
            "❌ {} failed validation ({} issues):",
            filename,
            errors.len()
        );
        let mut has_fatal = false;

        for err in errors.iter().take(12) {
            let level = match err.level {
                fastxml::ErrorLevel::Warning => "WARN",
                fastxml::ErrorLevel::Error => {
                    has_fatal = true;
                    "ERROR"
                }
                fastxml::ErrorLevel::Fatal => {
                    has_fatal = true;
                    "FATAL"
                }
            };
            let line = err
                .line()
                .map(|l| l.to_string())
                .unwrap_or_else(|| "?".to_string());

            println!("   [{}] line {}: {}", level, line, err.message);
        }

        if errors.len() > 12 {
            println!("   ... and {} more issues", errors.len() - 12);
        }

        if has_fatal {
            anyhow::bail!("Validation failed for {}", filename);
        } else {
            println!("⚠️  Only warnings — treated as passed");
            Ok(())
        }
    }
}
