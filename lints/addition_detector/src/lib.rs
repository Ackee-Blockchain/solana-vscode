#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use rustc_hir::{BinOpKind, Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};

dylint_linting::declare_late_lint! {
    /// ### What it does
    /// Detects addition operations in your code
    ///
    /// ### Why is this bad?
    /// This is a simple test lint to verify dylint integration works
    ///
    /// ### Example
    ///
    /// ```rust
    /// let x = 1 + 2; // Warning: addition detected
    /// ```
    pub ADDITION_DETECTOR,
    Warn,
    "detects addition operations"
}

impl<'tcx> LateLintPass<'tcx> for AdditionDetector {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if let ExprKind::Binary(op, _, _) = expr.kind {
            if op.node == BinOpKind::Add {
                clippy_utils::diagnostics::span_lint(
                    cx,
                    ADDITION_DETECTOR,
                    expr.span,
                    "addition operation detected",
                );
            }
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
