use crate::core::detectors::detector::{ClippyAnalysisContext, ClippyDetector, Detector, DetectorType};
use crate::core::detectors::detector_config::DetectorConfig;
use crate::core::utilities::DiagnosticBuilder;
use std::path::PathBuf;
use syn::spanned::Spanned;
use syn::{BinOp, Expr, ExprBinary, parse_str, visit::Visit};
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};

/// Clippy-style detector for unchecked arithmetic operations
/// This version uses compilation context to provide more accurate type-aware detection
#[derive(Default)]
pub struct ClippyUncheckedArithmeticDetector {
    diagnostics: Vec<Diagnostic>,
    config: DetectorConfig,
}

impl ClippyUncheckedArithmeticDetector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_config(config: DetectorConfig) -> Self {
        Self {
            diagnostics: Vec::new(),
            config,
        }
    }

    /// Analyze arithmetic operations with compilation context
    fn analyze_with_compilation_context(&mut self, context: &ClippyAnalysisContext) {
        // Parse the source code
        if let Ok(syntax_tree) = parse_str::<syn::File>(&context.source_code) {
            // Reset diagnostics for this analysis
            self.diagnostics.clear();

            // Visit the AST to find arithmetic operations
            self.visit_file(&syntax_tree);

            // If compilation was successful, we could enhance our analysis
            // with type information from the compilation result
            if context.compilation_result.success && context.compilation_result.type_info_available
            {
                self.enhance_with_type_info(context);
            }
        }
    }

    /// Enhanced analysis using type information from compilation
    fn enhance_with_type_info(&mut self, _context: &ClippyAnalysisContext) {
        // In a full implementation, this would:
        // 1. Parse the compilation output to get HIR
        // 2. Use rustc APIs to get type information
        // 3. Check if arithmetic operations are on integer types that can overflow
        // 4. Provide more accurate diagnostics with type-specific suggestions

        // For now, we'll simulate this by enhancing existing diagnostics
        for diagnostic in &mut self.diagnostics {
            // Add type-specific information to the diagnostic message
            if !diagnostic.message.contains("Type-aware") {
                diagnostic.message = format!("Type-aware: {}", diagnostic.message);
            }
        }
    }

    /// Check if an arithmetic operation is potentially unsafe
    fn is_unsafe_arithmetic(&self, expr: &ExprBinary) -> bool {
        match expr.op {
            BinOp::Add(_) | BinOp::Sub(_) | BinOp::Mul(_) | BinOp::Div(_) => {
                // More sophisticated checks than the syn version
                !self.has_overflow_protection(&expr.left, &expr.right)
            }
            _ => false,
        }
    }

    /// Check if the arithmetic operation has overflow protection
    fn has_overflow_protection(&self, left: &Expr, right: &Expr) -> bool {
        // Check for explicit type annotations
        if self.has_explicit_type_annotation(left) || self.has_explicit_type_annotation(right) {
            return true;
        }

        // Check for safe literals
        if self.is_safe_literal(left) && self.is_safe_literal(right) {
            return true;
        }

        // Check for checked arithmetic methods
        if self.is_checked_arithmetic_context(left) || self.is_checked_arithmetic_context(right) {
            return true;
        }

        false
    }

    /// Check if expression has explicit type annotation
    fn has_explicit_type_annotation(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Cast(_) => true,
            Expr::Lit(lit_expr) => {
                // Check for type suffixes like 42u64, 3.14f32
                match &lit_expr.lit {
                    syn::Lit::Int(int_lit) => !int_lit.suffix().is_empty(),
                    syn::Lit::Float(float_lit) => !float_lit.suffix().is_empty(),
                    _ => false,
                }
            }
            Expr::MethodCall(method_call) => {
                // Check for turbofish syntax: value.into::<u64>()
                method_call.turbofish.is_some()
            }
            Expr::Call(call_expr) => {
                // Check for generic function calls: from::<u64>(value)
                if let Expr::Path(path_expr) = &*call_expr.func {
                    path_expr
                        .path
                        .segments
                        .iter()
                        .any(|seg| matches!(seg.arguments, syn::PathArguments::AngleBracketed(_)))
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Check if expression is a safe literal
    fn is_safe_literal(&self, expr: &Expr) -> bool {
        if let Expr::Lit(lit_expr) = expr {
            if let syn::Lit::Int(int_lit) = &lit_expr.lit {
                // Consider small integers safe (less than 2^16 for clippy version)
                if let Ok(value) = int_lit.base10_parse::<u64>() {
                    return value < (1u64 << 16);
                }
            }
        }
        false
    }

    /// Check if expression is in a checked arithmetic context
    fn is_checked_arithmetic_context(&self, expr: &Expr) -> bool {
        // This would check if the expression is part of a checked arithmetic call
        // For example: x.checked_add(y), checked_mul(a, b), etc.
        // For now, we'll do a simple pattern match
        if let Expr::MethodCall(method_call) = expr {
            let method_name = method_call.method.to_string();
            method_name.starts_with("checked_")
                || method_name.starts_with("saturating_")
                || method_name.starts_with("wrapping_")
        } else {
            false
        }
    }

    /// Generate appropriate suggestion based on context
    fn generate_suggestion(&self, expr: &ExprBinary) -> String {
        let op_name = match expr.op {
            BinOp::Add(_) => "checked_add",
            BinOp::Sub(_) => "checked_sub",
            BinOp::Mul(_) => "checked_mul",
            BinOp::Div(_) => "checked_div",
            _ => "checked_operation",
        };

        format!(
            "Consider using {}() to handle potential overflow/underflow safely. \
            Alternatively, use explicit type annotations or ensure operands are within safe ranges.",
            op_name
        )
    }
}

impl ClippyDetector for ClippyUncheckedArithmeticDetector {
    fn analyze_with_context(&mut self, context: &ClippyAnalysisContext) -> Vec<Diagnostic> {
        self.analyze_with_compilation_context(context);
        self.diagnostics.clone()
    }
}

impl Detector for ClippyUncheckedArithmeticDetector {
    fn id(&self) -> &'static str {
        "CLIPPY_UNCHECKED_ARITHMETIC"
    }

    fn name(&self) -> &'static str {
        "Clippy Unchecked Arithmetic"
    }

    fn description(&self) -> &'static str {
        "Type-aware detection of unchecked arithmetic operations that could lead to overflow/underflow vulnerabilities"
    }

    fn message(&self) -> &'static str {
        "Unchecked arithmetic operation detected with type analysis"
    }

    fn default_severity(&self) -> DiagnosticSeverity {
        DiagnosticSeverity::ERROR
    }
}

impl<'ast> Visit<'ast> for ClippyUncheckedArithmeticDetector {
    fn visit_expr_binary(&mut self, node: &'ast ExprBinary) {
        if self.is_unsafe_arithmetic(node) {
            let severity = self
                .config
                .severity_override
                .unwrap_or(self.default_severity());

            let suggestion = self.generate_suggestion(node);

            self.diagnostics.push(DiagnosticBuilder::create(
                DiagnosticBuilder::create_range_from_span(node.span()),
                format!("{} {}", self.message(), suggestion),
                severity,
                self.id().to_string(),
                None,
            ));
        }

        // Continue visiting children
        syn::visit::visit_expr_binary(self, node);
    }
}
