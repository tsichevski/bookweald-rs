use anyhow::{Context, Result};
use libxml::parser::Parser;
use libxml::schemas::{SchemaParserContext, SchemaValidationContext};
use rayon::prelude::*;
use std::path::PathBuf;

pub fn validate(inputs: &[PathBuf], explicit_xsd: Option<&str>, dry_run: bool) {
    let (successes, errors): (Vec<_>, Vec<_>) = inputs
        .par_iter()
        .map(|path| {
            let filename = path
                .file_name()
                .and_then(|s| s.to_str())
                .ok_or_else(|| anyhow::anyhow!("Invalid filename: {}", path.display()))?;

            //tracing::debug!("🔍 Validating: {}", filename);

            // Parse document (this already checks well-formedness)
            let parser = Parser::default();
            let doc = parser
                .parse_file(path.to_str().unwrap_or(""))
                .context("Failed to parse XML document — not well-formed")?;

            if let Some(xsd) = explicit_xsd {
                // Full XSD validation
                let mut schema_ctx = SchemaParserContext::from_file(xsd);
                let mut validation_ctx = SchemaValidationContext::from_parser(&mut schema_ctx)
                    .map_err(|e| anyhow::anyhow!("Failed to create validation context: {:?}", e))?;

                match validation_ctx.validate_document(&doc) {
                    Ok(()) => {
                        tracing::debug!("✅ {} is valid according to XSD", filename);
                        Ok(())
                    }
                    Err(errors) => {
                        tracing::debug!(
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
                            tracing::debug!(
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
                tracing::debug!("✅ {} is well-formed and conforms to XML rules", filename);
                Ok(())
            }
        })
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

    if dry_run {
        tracing::info!("[dry-run] No files or directories were changed");
    }
}
