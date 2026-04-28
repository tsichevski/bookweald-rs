// src/validate.rs
use crate::config::BookwealdConfig;
use anyhow::{Context, Result};
use libxml::parser::Parser;
use libxml::schemas::{SchemaParserContext, SchemaValidationContext};
use std::path::Path;

pub fn validate(path: &Path, explicit_xsd: Option<&str>) -> Result<()> {
    let filename = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow::anyhow!("Invalid filename: {}", path.display()))?;

    println!("🔍 Validating with libxml2: {}", filename);

    // 1. Parse the FB2 document
    let parser = Parser::default();
    let doc = parser
        .parse_file(path.to_str().unwrap_or(""))
        .context("Failed to parse XML document")?;

    // 2. Load schema
    let mut schema_ctx = if let Some(xsd) = explicit_xsd {
        println!("   Using explicit schema: {}", xsd);
        SchemaParserContext::from_file(xsd)
    } else {
        println!("   Looking up schema from config.json...");
        let config = BookwealdConfig::load()?;
        let fb2_ns = "http://www.gribuser.ru/xml/fictionbook/2.0";

        if let Some(schema_path) = config.get_schema_for_namespace(fb2_ns) {
            println!("   Using mapped schema: {}", schema_path);
            SchemaParserContext::from_file(&schema_path)
        } else {
            anyhow::bail!(
                "No schema mapping found in config.json for FictionBook. Use --xsd flag."
            );
        }
    };

    // 3. Create validation context
    let mut validation_ctx = SchemaValidationContext::from_parser(&mut schema_ctx)
        .map_err(|e| anyhow::anyhow!("Failed to create validation context: {:?}", e))?;

    // 4. Validate
    match validation_ctx.validate_document(&doc) {
        Ok(()) => {
            println!("✅ {} is valid (libxml2)", filename);
            Ok(())
        }
        Err(errors) => {
            println!(
                "❌ {} failed validation ({} errors):",
                filename,
                errors.len()
            );
            for err in errors.iter().take(15) {
                let line = err.line.map_or("?".to_string(), |l| l.to_string());
                // Fixed: clone() because message is Option<String>
                let message = err
                    .message
                    .clone()
                    .unwrap_or_else(|| "Unknown error".to_string());

                println!("   [ERROR] line {}: {}", line, message);
            }
            if errors.len() > 15 {
                println!("   ... and {} more errors", errors.len() - 15);
            }
            anyhow::bail!("libxml2 validation failed for {}", filename);
        }
    }
}
