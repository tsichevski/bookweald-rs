//! FB2 / XML validation module (streaming XSD + well-formedness check).
//!
//! Heavy validation runs in parallel, but each file's error report is built
//! into a single string and emitted with **one** tracing call to minimise
//! interleaving from concurrent debug! output.

use anyhow::{Context, Result};
use libxml::error::StructuredError;
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

            // Parse document (checks well-formedness)
            let parser = Parser::default();
            let doc = match parser
                .parse_file(path.to_str().unwrap_or(""))
                .context("Failed to parse XML document — not well-formed")
            {
                Ok(doc) => doc,
                Err(e) => {
                    tracing::error!("❌ {}: {}", filename, e);
                    return Err(e);
                }
            };

            if let Some(xsd) = explicit_xsd {
                let mut schema_ctx = SchemaParserContext::from_file(xsd);
                let mut validation_ctx = match SchemaValidationContext::from_parser(&mut schema_ctx)
                {
                    Ok(ctx) => ctx,
                    Err(errors) => {
                        tracing::error!(
                            "{}",
                            build_schema_validation_error_report(&filename, errors)
                        );
                        anyhow::bail!("XSD validation failed for {}", filename);
                    }
                };

                match validation_ctx.validate_document(&doc) {
                    Ok(()) => {
                        tracing::debug!("✅ {} is valid according to XSD", filename);
                        Ok(())
                    }
                    Err(errors) => {
                        tracing::error!(
                            "{}",
                            build_schema_validation_error_report(&filename, errors)
                        );
                        anyhow::bail!("XSD validation failed for {}", filename);
                    }
                }
            } else {
                // Only well-formedness
                tracing::debug!("✅ {} is well-formed", filename);
                Ok(())
            }
        })
        .partition(Result::is_ok);

    let num_success = successes.len();
    let num_errors = errors.len();

    tracing::info!(
        "Validation completed: {} file(s) processed ({} OK, {} failed)",
        inputs.len(),
        num_success,
        num_errors
    );

    if dry_run {
        tracing::info!("[dry-run] No files were modified");
    }
}

fn build_schema_validation_error_report(filename: &str, errors: Vec<StructuredError>) -> String {
    // === Build full report as ONE string and emit atomically ===
    let mut report = format!(
        "❌ {} failed XSD validation ({} errors):\n",
        filename,
        errors.len()
    );

    for err in errors.iter().take(15) {
        let line = err.line.map_or_else(|| "?".to_string(), |l| l.to_string());
        let msg = err
            .message
            .clone()
            .unwrap_or_else(|| "Unknown error".to_string());
        report.push_str(&format!("   [ERROR] line {}: {}\n", line, msg));
    }

    if errors.len() > 15 {
        report.push_str(&format!("   ... and {} more errors\n", errors.len() - 15));
    }
    report
}
