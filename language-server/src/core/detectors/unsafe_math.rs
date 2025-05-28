use super::detector::Detector;
use crate::core::utilities::DiagnosticBuilder;
use syn::spanned::Spanned;
use syn::{BinOp, Expr, ExprBinary, parse_str, visit::Visit};
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};

pub struct UnsafeMathDetector {
    diagnostics: Vec<Diagnostic>,
}

impl UnsafeMathDetector {
    pub fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
        }
    }
}

impl Detector for UnsafeMathDetector {
    fn id(&self) -> &'static str {
        "UNSAFE_ARITHMETIC"
    }

    fn name(&self) -> &'static str {
        "Unsafe Math Operations"
    }

    fn description(&self) -> &'static str {
        "Detects unchecked arithmetic operations that could lead to overflow/underflow vulnerabilities"
    }

    fn message(&self) -> &'static str {
        "Unchecked arithmetic operation detected. Consider using checked_add(), checked_sub(), checked_mul(), or checked_div() to prevent overflow/underflow."
    }

    fn default_severity(&self) -> DiagnosticSeverity {
        DiagnosticSeverity::ERROR
    }

    fn analyze(&mut self, content: &str) -> Vec<Diagnostic> {
        if let Ok(syntax_tree) = parse_str::<syn::File>(content) {
            self.visit_file(&syntax_tree);
        }

        self.diagnostics.clone()
    }

    fn should_run(&self, content: &str) -> bool {
        // Run on Rust files that contain arithmetic operations and anchor imports
        if !(content.contains("anchor_lang") || content.contains("anchor_spl")) {
            return false;
        }

        // Look for arithmetic operations in more specific contexts to avoid false positives
        // from import statements like "use anchor_lang::prelude::*;"
        let lines: Vec<&str> = content.lines().collect();
        for line in lines {
            let trimmed = line.trim();
            // Skip import lines and comments
            if trimmed.starts_with("use ") || trimmed.starts_with("//") || trimmed.starts_with("/*") {
                continue;
            }

            // Look for arithmetic operators in actual code
            if trimmed.contains(" + ") || trimmed.contains(" - ") ||
               trimmed.contains(" * ") || trimmed.contains(" / ") ||
               trimmed.contains("+=") || trimmed.contains("-=") ||
               trimmed.contains("*=") || trimmed.contains("/=") {
                return true;
            }
        }

        false
    }
}

impl<'ast> Visit<'ast> for UnsafeMathDetector {
    fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
        if let BinOp::Add(_) = node.op {
            self.diagnostics.push(DiagnosticBuilder::create(
                DiagnosticBuilder::create_range_from_span(node.span()),
                self.message().to_string(),
                self.default_severity(),
                self.id().to_string(),
                None,
            ));
        }

        // Continue visiting children
        syn::visit::visit_expr_binary(self, node);
    }
}
