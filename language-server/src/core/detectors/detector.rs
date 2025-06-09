use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};

/// Base trait for all security detectors in Anchor programs
pub trait Detector: Send + Sync {
    /// Unique identifier for this detector
    fn id(&self) -> &'static str;

    /// Human-readable name for this detector
    fn name(&self) -> &'static str;
    /// Description of what this detector checks for
    fn description(&self) -> &'static str;
    /// Message for detection
    fn message(&self) -> &'static str;

    /// Severity level for diagnostics produced by this detector
    fn default_severity(&self) -> DiagnosticSeverity;

    /// Analyze the given content and return any security issues found
    fn analyze(&mut self, content: &str) -> Vec<Diagnostic>;
}
