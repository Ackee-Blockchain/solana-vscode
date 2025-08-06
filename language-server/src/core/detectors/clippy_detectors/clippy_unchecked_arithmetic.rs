use clippy_utils::sym::diagnostics;
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};

use crate::core::detectors::clippy_analyzer::ClippyAnalyzer;
use crate::core::detectors::detector::{
    ClippyAnalysisContext, ClippyDetector, Detector, DetectorType,
};

pub struct ClippyUncheckedArithmeticDetector {
    analyzer: Option<ClippyAnalyzer>,
}

impl Default for ClippyUncheckedArithmeticDetector {
    fn default() -> Self {
        Self::new()
    }
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

impl ClippyUncheckedArithmeticDetector {
    pub fn new() -> Self {
        Self {
            analyzer: Some(ClippyAnalyzer::new()),
        }
    }

    pub fn with_analyzer(analyzer: ClippyAnalyzer) -> Self {
        Self {
            analyzer: Some(analyzer),
        }
    }
}

impl ClippyDetector for ClippyUncheckedArithmeticDetector {
    fn detector_type(&self) -> DetectorType {
        DetectorType::Clippy
    }

    fn analyze_with_context(&mut self, context: &ClippyAnalysisContext) -> Vec<Diagnostic> {
        self.analyzer.unwrap().analyze_with_clippy(context)
    }
}
