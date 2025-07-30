use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};

use crate::core::detectors::detector::{
    ClippyAnalysisContext, ClippyDetector, Detector, DetectorType,
};

#[derive(Default, Clone)]
pub struct ClippyUncheckedArithmeticDetector {
    diagnostics: Vec<Diagnostic>,
}

impl Detector for ClippyUncheckedArithmeticDetector {
    fn id(&self) -> &'static str {
        "CLIPPY_UNCHECKED_ARITHMETIC"
    }
    fn name(&self) -> &'static str {
        "Unchecked Arithmetic"
    }
    fn description(&self) -> &'static str {
        "Detects unchecked arithmetic that may overflow."
    }
    fn message(&self) -> &'static str {
        "Use checked operations to prevent overflow."
    }
    fn default_severity(&self) -> DiagnosticSeverity {
        DiagnosticSeverity::ERROR
    }
}

impl ClippyDetector for ClippyUncheckedArithmeticDetector {
    fn detector_type(&self) -> DetectorType {
        DetectorType::Clippy
    }

    fn analyze_with_context(&mut self, context: &ClippyAnalysisContext) -> Vec<Diagnostic> {
        todo!()
    }
}
