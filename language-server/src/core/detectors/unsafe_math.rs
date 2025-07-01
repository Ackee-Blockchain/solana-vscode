use super::detector::Detector;
use super::detector_config::DetectorConfig;
use crate::core::utilities::DiagnosticBuilder;
use syn::spanned::Spanned;
use syn::{BinOp, parse_str, visit::Visit};
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};

#[derive(Default)]
pub struct UnsafeMathDetector {
    diagnostics: Vec<Diagnostic>,
    config: DetectorConfig,
}

impl UnsafeMathDetector {
    #[allow(dead_code)]
    pub fn with_config(config: DetectorConfig) -> Self {
        Self {
            diagnostics: Vec::new(),
            config,
        }
    }

    /// Check if an expression has explicit type casting or annotation
    fn has_explicit_type(&self, expr: &syn::Expr) -> bool {
        match expr {
            // Type casting: expr as Type
            syn::Expr::Cast(_) => true,
            // Literal with type suffix: 42u64, 3.14f32
            syn::Expr::Lit(lit_expr) => {
                if let syn::Lit::Int(int_lit) = &lit_expr.lit {
                    !int_lit.suffix().is_empty()
                } else if let syn::Lit::Float(float_lit) = &lit_expr.lit {
                    !float_lit.suffix().is_empty()
                } else {
                    false
                }
            }
            // Method call with explicit turbo fish: value.into::<u64>()
            syn::Expr::MethodCall(method_call) => method_call.turbofish.is_some(),
            // Function call with explicit generics: from::<u64>(value)
            syn::Expr::Call(call_expr) => {
                if let syn::Expr::Path(path_expr) = &*call_expr.func {
                    path_expr
                        .path
                        .segments
                        .iter()
                        .any(|seg| seg.arguments != syn::PathArguments::None)
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Check if an expression is a safe literal (small integer)
    fn is_safe_literal(&self, expr: &syn::Expr) -> bool {
        if let syn::Expr::Lit(lit_expr) = expr {
            if let syn::Lit::Int(int_lit) = &lit_expr.lit {
                // Consider small integers safe (less than 2^32)
                if let Ok(value) = int_lit.base10_parse::<u64>() {
                    return value < (1u64 << 32);
                }
            }
        }
        false
    }

    /// Check operand types for safety
    fn check_operand_types(&self, left: &syn::Expr, right: &syn::Expr) -> bool {
        let left_explicit = self.has_explicit_type(left);
        let right_explicit = self.has_explicit_type(right);
        let left_safe_literal = self.is_safe_literal(left);
        let right_safe_literal = self.is_safe_literal(right);

        left_explicit || right_explicit || (left_safe_literal && right_safe_literal)
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
        match node.op {
            BinOp::Add(_) | BinOp::Sub(_) | BinOp::Mul(_) | BinOp::Div(_) => {
                // Check if operands have explicit type annotations or are literals
                let is_type_safe = self.check_operand_types(&node.left, &node.right);

                // Only flag if we can't determine safe types
                if !is_type_safe {
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
            }
            _ => {}
        }

        // Continue visiting children
        syn::visit::visit_expr_binary(self, node);
    }
}
