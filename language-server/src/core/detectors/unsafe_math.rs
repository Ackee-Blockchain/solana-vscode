use super::detector::Detector;
use super::detector_config::DetectorConfig;
use crate::core::utilities::DiagnosticBuilder;
use syn::spanned::Spanned;
use syn::{BinOp, parse_str, visit::Visit};
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};

pub struct UnsafeMathDetector {
    diagnostics: Vec<Diagnostic>,
    config: DetectorConfig,
}

impl UnsafeMathDetector {
    pub fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
            config: DetectorConfig::default(),
        }
    }

    #[allow(dead_code)]
    pub fn with_config(config: DetectorConfig) -> Self {
        Self {
            diagnostics: Vec::new(),
            config,
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
        self.diagnostics.clear();

        // Run default detection logic
        if let Ok(syntax_tree) = parse_str::<syn::File>(content) {
            self.visit_file(&syntax_tree);
        }

        self.diagnostics.clone()
    }
}

impl<'ast> Visit<'ast> for UnsafeMathDetector {
    fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
        if let BinOp::Add(_) = node.op {
            let severity = self
                .config
                .severity_override
                .unwrap_or(self.default_severity());
            self.diagnostics.push(DiagnosticBuilder::create(
                DiagnosticBuilder::create_range_from_span(node.span()),
                self.message().to_string(),
                severity,
                self.id().to_string(),
                None,
            ));
        }

        // Continue visiting children
        syn::visit::visit_expr_binary(self, node);
    }
}
