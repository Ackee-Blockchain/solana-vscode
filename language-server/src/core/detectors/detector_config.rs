use tower_lsp::lsp_types::DiagnosticSeverity;

/// Configuration for detectors
#[derive(Debug, Clone)]
pub struct DetectorConfig {
    pub enabled: bool,
    pub severity_override: Option<DiagnosticSeverity>,
    pub custom_patterns: Vec<String>,
}

impl Default for DetectorConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            severity_override: None,
            custom_patterns: Vec::new(),
        }
    }
}