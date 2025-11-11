use serde::{Deserialize, Serialize};

/// A diagnostic message from dylint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DylintDiagnostic {
    pub file_name: String,
    pub line_start: usize,
    pub line_end: usize,
    pub column_start: usize,
    pub column_end: usize,
    pub message: String,
    pub code: String,
    pub level: String,
}

impl DylintDiagnostic {
    /// Convert to LSP Diagnostic
    pub fn to_lsp_diagnostic(&self) -> tower_lsp::lsp_types::Diagnostic {
        use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};

        let severity = match self.level.as_str() {
            "error" => Some(DiagnosticSeverity::ERROR),
            "warning" => Some(DiagnosticSeverity::WARNING),
            "note" | "help" => Some(DiagnosticSeverity::INFORMATION),
            _ => Some(DiagnosticSeverity::WARNING),
        };

        // LSP uses 0-based indexing, dylint uses 1-based
        let start = Position {
            line: (self.line_start.saturating_sub(1)) as u32,
            character: (self.column_start.saturating_sub(1)) as u32,
        };
        let end = Position {
            line: (self.line_end.saturating_sub(1)) as u32,
            character: (self.column_end.saturating_sub(1)) as u32,
        };

        Diagnostic {
            range: Range { start, end },
            severity,
            code: Some(tower_lsp::lsp_types::NumberOrString::String(
                self.code.clone(),
            )),
            source: Some("dylint".to_string()),
            message: self.message.clone(),
            ..Default::default()
        }
    }
}
