use clippy_utils::{rustc_hir, eq_expr_value};
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};

use crate::core::detectors::detector::{Detector, DetectorType};

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
    fn analyze_with_context(&mut self, _context: &ClippyAnalysisContext) -> Vec<Diagnostic> {
        vec![]
    } // If still needed
    fn get_diagnostics(&self) -> Vec<Diagnostic> {
        self.diagnostics.clone()
    }
}

impl<'tcx> LateLintPass<'tcx> for ClippyUncheckedArithmeticDetector {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if let ExprKind::Binary(op, left, right) = expr.kind {
            if matches!(op.node, BinOpKind::Add | BinOpKind::Sub | BinOpKind::Mul) {
                let left_ty = cx.typeck_results().expr_ty(left);
                let right_ty = cx.typeck_results().expr_ty(right);
                if is_integer_ty(cx, left_ty)
                    && is_integer_ty(cx, right_ty)
                    && !eq_expr_value(cx, left, right)
                {
                    // Example check
                    let diag = Diagnostic {
                        range: tower_lsp::lsp_types::Range::default(), // Add import if needed
                        severity: Some(self.default_severity()),
                        message: self.message().to_string(),
                        ..Default::default()
                    };
                    self.diagnostics.push(diag);
                }
            }
        }
    }
}
