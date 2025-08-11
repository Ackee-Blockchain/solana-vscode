use super::detector::Detector;
use super::detector_config::DetectorConfig;
use crate::core::utilities::DiagnosticBuilder;
use std::path::PathBuf;
use syn::spanned::Spanned;
use syn::{
    parse_str, visit::Visit, Expr, ExprAssign, ExprField, ExprLit, ExprMethodCall, Lit, UnOp,
};
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};

#[derive(Default)]
pub struct ManualLamportsZeroingDetector {
    diagnostics: Vec<Diagnostic>,
    config: DetectorConfig,
}

impl ManualLamportsZeroingDetector {
    #[allow(dead_code)]
    pub fn with_config(config: DetectorConfig) -> Self {
        Self {
            diagnostics: Vec::new(),
            config,
        }
    }

    /// Peel common wrappers so we can reason about the "real" expression:
    /// - Parentheses: ( .. )
    /// - References: &expr
    /// - Deref: *expr
    /// - Try: expr?  (Expr::Try)
    fn strip_wrappers<'a>(&self, mut expr: &'a Expr) -> &'a Expr {
        loop {
            expr = match expr {
                Expr::Paren(p) => &p.expr,
                Expr::Reference(r) => &r.expr,
                Expr::Unary(u) if matches!(u.op, UnOp::Deref(_)) => &u.expr, // *
                Expr::Try(t) => &t.expr, // ?
                _ => break expr,
            };
        }
    }

    /// Return true if the expression represents an access to lamports:
    /// - foo.lamports
    /// - foo.lamports()   (some code uses a method accessor)
    /// - foo.lamports.borrow_mut()
    /// - foo.try_borrow_mut_lamports()
    fn is_lamports_access(&self, expr: &Expr) -> bool {
        let e = self.strip_wrappers(expr);

        match e {
            // 1) foo.lamports
            Expr::Field(ExprField {
                member: syn::Member::Named(ident),
                ..
            }) if ident == "lamports" => true,

            // 2) foo.lamports() — allow method named `lamports`
            Expr::MethodCall(ExprMethodCall { method, .. }) if method == "lamports" => true,

            // 3) foo.lamports.borrow_mut() — the receiver of borrow_mut() must be a lamports access
            Expr::MethodCall(ExprMethodCall {
                method,
                receiver,
                ..
            }) if method == "borrow_mut" => self.is_lamports_access(receiver),

            // 4) foo.try_borrow_mut_lamports()
            Expr::MethodCall(ExprMethodCall { method, .. })
                if method == "try_borrow_mut_lamports" =>
            {
                true
            }

            _ => false,
        }
    }

    /// Return true if expression is the integer literal 0 (possibly wrapped).
    fn is_zero_literal(&self, expr: &Expr) -> bool {
        match self.strip_wrappers(expr) {
            Expr::Lit(ExprLit {
                lit: Lit::Int(lit_int),
                ..
            }) => lit_int.base10_digits() == "0",
            _ => false,
        }
    }

    /// Detect `lamports = 0`
    fn is_lamports_zero_assignment(&self, assign: &ExprAssign) -> bool {
        self.is_lamports_access(&assign.left) && self.is_zero_literal(&assign.right)
    }

    /// Detect method forms that set lamports to zero, e.g. `account.set_lamports(0)`
    fn is_lamports_zero_method_call(&self, method_call: &ExprMethodCall) -> bool {
        if method_call.method == "set_lamports" {
            if let Some(arg) = method_call.args.first() {
                return self.is_zero_literal(arg);
            }
        }
        false
    }

    /// Detect any manual lamports zeroing pattern
    fn is_manual_lamports_pattern(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Assign(assign) => self.is_lamports_zero_assignment(assign),
            Expr::MethodCall(mc) => self.is_lamports_zero_method_call(mc),
            _ => false,
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

    fn analyze(&mut self, content: &str, _file_path: Option<&PathBuf>) -> Vec<Diagnostic> {
        self.diagnostics.clear();

        if let Ok(syntax_tree) = parse_str::<syn::File>(content) {
            self.visit_file(&syntax_tree);
        }

        self.diagnostics.clone()
    }
}

impl<'ast> Visit<'ast> for ManualLamportsZeroingDetector {
    fn visit_expr(&mut self, node: &'ast Expr) {
        // Check manual lamports zeroing patterns like:
        // **ctx.accounts.victim.try_borrow_mut_lamports()? = 0;
        // **acct.lamports.borrow_mut() = 0;
        // acct.set_lamports(0);
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

        // Continue traversal
        syn::visit::visit_expr(self, node);
    }
}
