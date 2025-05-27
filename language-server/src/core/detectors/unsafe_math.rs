use super::detector::Detector;
use syn::{parse_str, visit, visit::Visit, BinOp, Expr, ExprBinary};
use syn::spanned::Spanned;
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};
use crate::core::utilities::DiagnosticBuilder;

pub struct UnsafeMathDetector {
    diagnostics: Vec<Diagnostic>,
}

impl Detector for UnsafeMathDetector {
    const ID: &'static str = "UNSAFE_ARITHMETIC";
    const NAME: &'static str = "Unsafe Math Operations";
    const DESCRIPTION: &'static str =
        "Detects unchecked arithmetic operations that could lead to overflow/underflow vulnerabilities";
    const MESSAGE: &'static str =
        "Unchecked arithmetic operation detected. Consider using checked_add(), checked_sub(), checked_mul(), or checked_div() to prevent overflow/underflow.";
    const DEFAULT_SEVERITY: DiagnosticSeverity = DiagnosticSeverity::ERROR;

    fn analyze(&mut self, content: &str) -> Vec<Diagnostic> {
        if let Ok(syntax_tree) = parse_str::<syn::File>(content) {
            self.visit_file(&syntax_tree);
        }

        self.diagnostics.clone()
    }

    fn should_run(&self, content: &str) -> bool {
        // Run on Rust files that contain arithmetic operations and anchor imports
        (content.contains("anchor_lang") || content.contains("anchor_spl"))
            && (content.contains('+')
            || content.contains('-')
            || content.contains('*')
            || content.contains('/'))
    }
}

impl<'ast> Visit<'ast> for UnsafeMathDetector {
    fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
        if let BinOp::Add(_) = node.op {
            self.diagnostics.push(DiagnosticBuilder::create(
                DiagnosticBuilder::create_range_from_span(node.span()),
                Self::MESSAGE.to_string(),
                Self::DEFAULT_SEVERITY,
                Self::ID.to_string(),
                None,
            ));
        }

        // Continue visiting children
        visit::visit_expr_binary(self, node);
    }
}
