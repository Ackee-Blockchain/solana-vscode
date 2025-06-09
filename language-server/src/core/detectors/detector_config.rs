use serde::{Deserialize, Serialize};
use tower_lsp::lsp_types::DiagnosticSeverity;

/// Configuration for detectors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectorConfig {
    pub enabled: bool,
    pub severity_override: Option<DiagnosticSeverity>,
}

impl Default for DetectorConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            severity_override: None,
        }
    }
}

impl DetectorConfig {
    /// Create a config that disables the detector
    #[allow(dead_code)]
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    /// Create a config with custom severity
    #[allow(dead_code)]
    pub fn with_severity(severity: DiagnosticSeverity) -> Self {
        Self {
            severity_override: Some(severity),
            ..Default::default()
        }
    }
}
