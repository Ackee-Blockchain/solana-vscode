#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_span;

use rustc_hir as hir;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::source_map::SourceMap;
use std::sync::{Arc, Mutex};

dylint_linting::declare_late_lint! {
    /// ### What it does
    /// Detects any addition operation (super simple test lint)
    ///
    /// ### Why is this bad?
    /// This is just a test to verify the lint works
    ///
    /// ### Example
    ///
    /// ```rust
    /// let x = 1 + 2;
    /// ```
    pub NO_ADDITION,
    Warn,
    "detects addition operations (test lint)"
}

// Diagnostic structure that can be used by LSP
#[derive(Debug, Clone)]
pub struct CapturedDiagnostic {
    pub message: String,
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

// Thread-safe storage for diagnostics
pub type DiagnosticCollector = Arc<Mutex<Vec<CapturedDiagnostic>>>;

impl<'tcx> LateLintPass<'tcx> for NoAddition {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx hir::Expr<'tcx>) {
        // SKIP MACRO EXPANSIONS - only lint user-written code
        if expr.span.from_expansion() {
            return;
        }

        if let hir::ExprKind::Binary(op, _, _) = &expr.kind {
            if op.node == hir::BinOpKind::Add {
                // Emit the lint (this shows up in cargo output)
                cx.lint(NO_ADDITION, |diag| {
                    diag.primary_message("found an addition operation");
                    diag.span(expr.span);
                });

                // ALSO capture it for programmatic access (for LSP)
                // Note: To access the collector, you'd need to pass it through
                // the lint struct. See the comment below for how to do this.
            }
        }
    }
}

// HOW TO CAPTURE DIAGNOSTICS IN LSP:
//
// When you integrate this into your LSP, you'll:
// 1. Build the lint as a .so/.dylib file
// 2. Use `cargo dylint` or `dylint_driver` with this lint
// 3. Capture the JSON output using --message-format=json
// 4. Parse the JSON to extract diagnostics
//
// OR (better approach):
// 1. Don't use Dylint at all - use your existing ClippyDetector pattern
// 2. Port the lint logic from this file into unsafe_math_clippy.rs
// 3. Use the Arc<Mutex<Vec<Diagnostic>>> pattern you already have
// 4. Let Cargo handle the compilation via your existing infrastructure

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
