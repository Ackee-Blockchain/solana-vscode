use tower_lsp::lsp_types::DiagnosticSeverity;
use serde::{Deserialize, Serialize};

/// Configuration for detectors
#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl DetectorConfig {
    /// Create a config that disables the detector
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    /// Create a config with custom severity
    pub fn with_severity(severity: DiagnosticSeverity) -> Self {
        Self {
            severity_override: Some(severity),
            ..Default::default()
        }
    }

    /// Create a config with custom patterns
    pub fn with_patterns(patterns: Vec<String>) -> Self {
        Self {
            custom_patterns: patterns,
            ..Default::default()
        }
    }
}
