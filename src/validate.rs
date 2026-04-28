use anyhow::{Context, Result};
use libxml::parser::Parser;
use libxml::schemas::{SchemaParserContext, SchemaValidationContext};
use std::fs;
use std::path::Path;

pub fn validate(path: &Path, explicit_xsd: Option<&str>) -> Result<()> {
    if path.is_dir() {
        return validate_directory(path, explicit_xsd);
    }

    let filename = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow::anyhow!("Invalid filename: {}", path.display()))?;

    println!("🔍 Validating: {}", filename);

    // Parse document (this already checks well-formedness)
    let parser = Parser::default();
    let doc = parser
        .parse_file(path.to_str().unwrap_or(""))
        .context("Failed to parse XML document — not well-formed")?;

    if let Some(xsd) = explicit_xsd {
        // Full XSD validation
        println!("   Using explicit schema: {}", xsd);
        let mut schema_ctx = SchemaParserContext::from_file(xsd);

        let mut validation_ctx = SchemaValidationContext::from_parser(&mut schema_ctx)
            .map_err(|e| anyhow::anyhow!("Failed to create validation context: {:?}", e))?;

        match validation_ctx.validate_document(&doc) {
            Ok(()) => {
                println!("✅ {} is valid according to XSD", filename);
                Ok(())
            }
            Err(errors) => {
                println!(
                    "❌ {} failed XSD validation ({} errors):",
                    filename,
                    errors.len()
                );
                for err in errors.iter().take(15) {
                    let line = err.line.map(|l| l.to_string());
                    let msg = err
                        .message
                        .clone()
                        .unwrap_or_else(|| "Unknown error".to_string());
                    println!(
                        "   [ERROR] line {}: {}",
                        line.as_deref().unwrap_or("?"),
                        msg
                    );
                }
                anyhow::bail!("XSD validation failed for {}", filename);
            }
        }
    } else {
        // XML Conformance only
        println!("   XML conformance check (well-formedness + basic structure)");
        println!("✅ {} is well-formed and conforms to XML rules", filename);
        Ok(())
    }
}

// Private recursive directory handler
fn validate_directory(dir: &Path, explicit_xsd: Option<&str>) -> Result<()> {
    let mut failed = 0;

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            if let Err(_) = validate_directory(&path, explicit_xsd) {
                failed += 1;
            }
        } else if path.is_file() {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_lowercase());

            if matches!(ext.as_deref(), Some("fb2") | Some("fb2.zip")) {
                println!();
                match validate(&path, explicit_xsd) {
                    Ok(()) => (),
                    Err(_) => failed += 1,
                }
            }
        }
    }

    if failed > 0 {
        anyhow::bail!(
            "{} file(s) failed validation under {}",
            failed,
            dir.display()
        );
    }
    Ok(())
}
