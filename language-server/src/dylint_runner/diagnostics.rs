use serde::{Deserialize, Serialize};
use std::path::Path;

/// Related information for a diagnostic (e.g., pointing to a field declaration or mutation site)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DylintRelatedInfo {
    pub file_name: String,
    pub line_start: usize,
    pub line_end: usize,
    pub column_start: usize,
    pub column_end: usize,
    pub message: String,
}

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
    pub related_information: Vec<DylintRelatedInfo>,
}

impl DylintDiagnostic {
    /// Convert to LSP Diagnostic.
    /// `workspace_root` is used to resolve relative file paths in related information.
    pub fn to_lsp_diagnostic(
        &self,
        workspace_root: Option<&Path>,
    ) -> tower_lsp::lsp_types::Diagnostic {
        use tower_lsp::lsp_types::{
            Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity, Location, Position,
            Range, Url,
        };

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

        let related_information = if self.related_information.is_empty() {
            None
        } else {
            let infos: Vec<DiagnosticRelatedInformation> = self
                .related_information
                .iter()
                .filter_map(|info| {
                    let path = Path::new(&info.file_name);
                    let abs_path = if path.is_absolute() {
                        path.to_path_buf()
                    } else if let Some(root) = workspace_root {
                        root.join(path)
                    } else {
                        return None;
                    };
                    let uri = Url::from_file_path(&abs_path).ok()?;
                    Some(DiagnosticRelatedInformation {
                        location: Location {
                            uri,
                            range: Range {
                                start: Position {
                                    line: (info.line_start.saturating_sub(1)) as u32,
                                    character: (info.column_start.saturating_sub(1)) as u32,
                                },
                                end: Position {
                                    line: (info.line_end.saturating_sub(1)) as u32,
                                    character: (info.column_end.saturating_sub(1)) as u32,
                                },
                            },
                        },
                        message: info.message.clone(),
                    })
                })
                .collect();
            if infos.is_empty() { None } else { Some(infos) }
        };

        Diagnostic {
            range: Range { start, end },
            severity,
            code: Some(tower_lsp::lsp_types::NumberOrString::String(
                self.code.clone(),
            )),
            source: Some("solana".to_string()),
            message: self.message.clone(),
            related_information,
            ..Default::default()
        }
    }
}
