use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};

/// Base trait for all security detectors in Anchor programs
pub trait Detector: Send + Sync {
    /// Unique identifier for this detector
    const ID: &'static str;

    /// Human-readable name for this detector
    const NAME: &'static str;

    /// Description of what this detector checks for
    const DESCRIPTION: &'static str;
    
    /// Message for detection
    const MESSAGE: &'static str;

    /// Severity level for diagnostics produced by this detector
    const DEFAULT_SEVERITY: DiagnosticSeverity = DiagnosticSeverity::ERROR;

    /// Analyze the given content and return any security issues found
    fn analyze(&mut self, content: &str) -> Vec<Diagnostic>;

    /// Check if this detector should run on the given content
    /// Can be used for performance optimization or file type filtering
    fn should_run(&self, content: &str) -> bool {
        // Default: run on all Rust files that contain anchor imports
        content.contains("anchor_lang") || content.contains("anchor_spl")
    }
}
