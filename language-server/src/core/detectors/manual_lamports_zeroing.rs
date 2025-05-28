use super::detector::Detector;
use super::detector_config::DetectorConfig;
use crate::core::utilities::DiagnosticBuilder;
use syn::spanned::Spanned;
use syn::{Expr, ExprAssign, ExprField, ExprLit, ExprMethodCall, Lit, parse_str, visit::Visit};
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};

pub struct ManualLamportsZeroingDetector {
    diagnostics: Vec<Diagnostic>,
    config: DetectorConfig,
}

impl ManualLamportsZeroingDetector {
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

    /// Check if an expression is accessing the lamports field
    fn is_lamports_access(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Field(ExprField { member, .. }) => {
                if let syn::Member::Named(ident) = member {
                    ident == "lamports"
                } else {
                    false
                }
            }
            Expr::MethodCall(ExprMethodCall { method, .. }) => method == "lamports",
            _ => false,
        }
    }

    /// Check if an expression is zero (literal 0)
    fn is_zero_literal(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Lit(ExprLit {
                lit: Lit::Int(lit_int),
                ..
            }) => lit_int.base10_digits() == "0",
            _ => false,
        }
    }

    /// Check if this is a lamports assignment to zero
    fn is_lamports_zero_assignment(&self, assign: &ExprAssign) -> bool {
        self.is_lamports_access(&assign.left) && self.is_zero_literal(&assign.right)
    }

    /// Check if this is a method call that sets lamports to zero
    fn is_lamports_zero_method_call(&self, method_call: &ExprMethodCall) -> bool {
        // Check for patterns like account.set_lamports(0) or **account.lamports.borrow_mut() = 0
        if method_call.method == "set_lamports" {
            if let Some(arg) = method_call.args.first() {
                return self.is_zero_literal(arg);
            }
        }
        false
    }

    /// Check if this is a manual lamports manipulation pattern
    fn is_manual_lamports_pattern(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Assign(assign) => self.is_lamports_zero_assignment(assign),
            Expr::MethodCall(method_call) => self.is_lamports_zero_method_call(method_call),
            _ => false,
        }
    }

    /// Check for custom patterns in the content
    fn check_custom_patterns(&mut self, content: &str) {
        for pattern in &self.config.custom_patterns {
            let mut start_pos = 0;
            while let Some(pos) = content[start_pos..].find(pattern) {
                let actual_pos = start_pos + pos;

                // Calculate line and column for the match
                let lines_before = content[..actual_pos].matches('\n').count();
                let line_start = content[..actual_pos]
                    .rfind('\n')
                    .map(|p| p + 1)
                    .unwrap_or(0);
                let column = actual_pos - line_start;

                // Create diagnostic for custom pattern
                let diagnostic = DiagnosticBuilder::create(
                    tower_lsp::lsp_types::Range {
                        start: tower_lsp::lsp_types::Position {
                            line: lines_before as u32,
                            character: column as u32,
                        },
                        end: tower_lsp::lsp_types::Position {
                            line: lines_before as u32,
                            character: (column + pattern.len()) as u32,
                        },
                    },
                    format!("Custom pattern '{}' detected. {}", pattern, self.message()),
                    self.config
                        .severity_override
                        .unwrap_or(self.default_severity()),
                    format!("{}_CUSTOM", self.id()),
                    None,
                );

                self.diagnostics.push(diagnostic);
                start_pos = actual_pos + pattern.len();
            }
        }
    }
}

impl Detector for ManualLamportsZeroingDetector {
    fn id(&self) -> &'static str {
        "MANUAL_LAMPORTS_ZEROING"
    }

    fn name(&self) -> &'static str {
        "Manual Lamports Zeroing"
    }

    fn description(&self) -> &'static str {
        "Detects manual lamports zeroing which can lead to incomplete account closure and potential security vulnerabilities"
    }

    fn message(&self) -> &'static str {
        "Manual lamports zeroing detected. Use proper account closure mechanisms like `close` or transfer lamports to another account instead of setting to zero."
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

        // Check custom patterns
        self.check_custom_patterns(content);

        self.diagnostics.clone()
    }

    fn should_run(&self, content: &str) -> bool {
        // Always run if custom patterns are configured
        if !self.config.custom_patterns.is_empty() {
            return content.contains("anchor_lang")
                || content.contains("solana_program")
                || content.contains("anchor_spl");
        }

        // Run on Anchor/Solana files that contain lamports references
        (content.contains("anchor_lang")
            || content.contains("solana_program")
            || content.contains("anchor_spl"))
            && (content.contains("lamports") || content.contains("set_lamports"))
    }
}

impl<'ast> Visit<'ast> for ManualLamportsZeroingDetector {
    fn visit_expr(&mut self, node: &'ast Expr) {
        // Check if this expression is a manual lamports zeroing pattern
        if self.is_manual_lamports_pattern(node) {
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
        syn::visit::visit_expr(self, node);
    }
}
