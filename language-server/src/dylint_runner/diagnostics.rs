use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, Position, Range};

#[derive(Debug, Clone)]
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
    pub fn to_lsp_diagnostic(&self) -> Diagnostic {
        Diagnostic {
            range: Range {
                start: Position {
                    line: (self.line_start.saturating_sub(1)) as u32, // LSP is 0-indexed
                    character: (self.column_start.saturating_sub(1)) as u32,
                },
                end: Position {
                    line: (self.line_end.saturating_sub(1)) as u32,
                    character: (self.column_end.saturating_sub(1)) as u32,
                },
            },
            severity: Some(self.severity()),
            code: Some(NumberOrString::String(self.code.clone())),
            source: Some("dylint".to_string()),
            message: self.message.clone(),
            ..Default::default()
        }
    }

    fn severity(&self) -> DiagnosticSeverity {
        match self.level.as_str() {
            "error" => DiagnosticSeverity::ERROR,
            "warning" => DiagnosticSeverity::WARNING,
            "note" | "help" => DiagnosticSeverity::INFORMATION,
            _ => DiagnosticSeverity::WARNING,
        }
    }
}

