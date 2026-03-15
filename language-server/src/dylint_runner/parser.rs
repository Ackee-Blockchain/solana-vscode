use super::diagnostics::{DylintDiagnostic, DylintRelatedInfo};
use anyhow::{Context, Result};
use log::debug;
use serde_json::Value;

/// Parse cargo check JSON output and extract lint diagnostics
/// Only accepts diagnostics from the specified lint codes (whitelist approach)
pub fn parse_json_output(
    stdout: &str,
    allowed_lint_codes: &[String],
) -> Result<Vec<DylintDiagnostic>> {
    let mut diagnostics = Vec::new();

    for line in stdout.lines() {
        // Skip empty lines
        if line.trim().is_empty() {
            continue;
        }

        // Parse JSON
        let json: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue, // Skip non-JSON lines
        };

        // Filter for compiler messages
        if json.get("reason").and_then(|r| r.as_str()) != Some("compiler-message") {
            continue;
        }

        // Extract message
        let message = match json.get("message") {
            Some(m) => m,
            None => continue,
        };

        // Check if it has a code (lints have codes)
        let code = message
            .get("code")
            .and_then(|c| c.get("code"))
            .and_then(|c| c.as_str());

        // WHITELIST: Only accept diagnostics from our loaded lints
        if let Some(code) = code
            && allowed_lint_codes
                .iter()
                .any(|allowed| code.contains(allowed))
        {
            // Parse into DylintDiagnostic
            if let Ok(diagnostic) = parse_diagnostic(message) {
                diagnostics.push(diagnostic);
            }
        }
    }

    Ok(diagnostics)
}

/// Parse a single diagnostic message
fn parse_diagnostic(message: &Value) -> Result<DylintDiagnostic> {
    // Get primary span
    let spans = message
        .get("spans")
        .and_then(|s| s.as_array())
        .context("No spans in message")?;

    let primary_span = spans
        .iter()
        .find(|s| s.get("is_primary") == Some(&Value::Bool(true)))
        .context("No primary span found")?;

    // Skip macro expansions
    if primary_span.get("expansion").is_some() && !primary_span.get("expansion").unwrap().is_null()
    {
        anyhow::bail!("Diagnostic from macro expansion (filtered)");
    }

    // Extract fields
    let file_name = primary_span
        .get("file_name")
        .and_then(|f| f.as_str())
        .context("No file_name")?
        .to_string();

    let line_start = primary_span
        .get("line_start")
        .and_then(|l| l.as_u64())
        .context("No line_start")? as usize;

    let line_end = primary_span
        .get("line_end")
        .and_then(|l| l.as_u64())
        .context("No line_end")? as usize;

    let column_start = primary_span
        .get("column_start")
        .and_then(|c| c.as_u64())
        .context("No column_start")? as usize;

    let column_end = primary_span
        .get("column_end")
        .and_then(|c| c.as_u64())
        .context("No column_end")? as usize;

    let msg = message
        .get("message")
        .and_then(|m| m.as_str())
        .context("No message text")?
        .to_string();

    let code = message
        .get("code")
        .and_then(|c| c.get("code"))
        .and_then(|c| c.as_str())
        .context("No code")?
        .to_uppercase(); // Display detector names in uppercase

    let level = message
        .get("level")
        .and_then(|l| l.as_str())
        .context("No level")?
        .to_string();

    // Parse children (notes/help) that have spans for related information
    let related_information = parse_related_information(message);
    debug!(
        "[Dylint Parser] Diagnostic '{}' has {} related info(s), children: {}",
        msg,
        related_information.len(),
        message
            .get("children")
            .map_or("none".to_string(), |c| format!("{}", c))
    );

    Ok(DylintDiagnostic {
        file_name,
        line_start,
        line_end,
        column_start,
        column_end,
        message: msg,
        code,
        level,
        related_information,
    })
}

/// Parse children of a diagnostic message to extract related information with spans
fn parse_related_information(message: &Value) -> Vec<DylintRelatedInfo> {
    let mut related = Vec::new();

    let children = match message.get("children").and_then(|c| c.as_array()) {
        Some(c) => c,
        None => return related,
    };

    for child in children {
        // Only include children that have spans (notes with locations)
        let child_spans = match child.get("spans").and_then(|s| s.as_array()) {
            Some(s) if !s.is_empty() => s,
            _ => continue,
        };

        let child_msg = match child.get("message").and_then(|m| m.as_str()) {
            Some(m) => m.to_string(),
            None => continue,
        };

        // Use the first span from the child
        if let Some(span) = child_spans.first() {
            let file_name = match span.get("file_name").and_then(|f| f.as_str()) {
                Some(f) => f.to_string(),
                None => continue,
            };

            let line_start = span.get("line_start").and_then(|l| l.as_u64()).unwrap_or(1) as usize;
            let line_end = span.get("line_end").and_then(|l| l.as_u64()).unwrap_or(1) as usize;
            let column_start = span
                .get("column_start")
                .and_then(|c| c.as_u64())
                .unwrap_or(1) as usize;
            let column_end = span.get("column_end").and_then(|c| c.as_u64()).unwrap_or(1) as usize;

            related.push(DylintRelatedInfo {
                file_name,
                line_start,
                line_end,
                column_start,
                column_end,
                message: child_msg,
            });
        }
    }

    related
}
