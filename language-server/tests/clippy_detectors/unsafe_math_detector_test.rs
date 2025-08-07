extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

use std::fs::write;
use std::path::PathBuf;
use std::sync::Mutex;

use rustc_driver::{Callbacks, Compilation, run_compiler};
use rustc_interface::{Config, interface::Compiler};
use rustc_lint::{LateContext, LateLintPass, LintPass, LintStore, Lint};
use rustc_middle::ty::TyCtxt;
use rustc_session::declare_lint;
use crate::clippy_detectors::unsafe_math_detector_test::rustc_lint::LintContext;

// The `test_lint` lint detects every expression in the code.
//
// ### Example
//
// ```rust
// fn main() {
//     let x = 1;
// }
// ```
//
// {{produces}}
//
// ### Explanation
//
// This is a test lint that warns on every expression to verify the linting infrastructure.
// It serves as a proof-of-concept and should be removed in production code. This lint is
// "allow" by default in practice but is set to "warn" here for testing purposes.
//
// [rustc-dev-guide]: https://rustc-dev-guide.rust-lang.org/
declare_lint! {
    pub TEST_LINT,
    Warn,
    "test lint that warns on every expression"
}

#[derive(Copy, Clone)]
struct ExampleLateLintPass;

impl LintPass for ExampleLateLintPass {
    fn name(&self) -> &'static str {
        "example_lint"
    }

    fn get_lints(&self) -> Vec<&'static Lint> {
        vec![&TEST_LINT]
    }
}

impl<'tcx> LateLintPass<'tcx> for ExampleLateLintPass {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx rustc_hir::Expr<'tcx>) {
        // Emit a lint warning and store the message
        let msg = "test lint triggered";
        cx.span_lint(&TEST_LINT, expr.span, |diag| {
            diag.primary_message(msg);
        });
        
        // Store the diagnostic message in the global DIAGNOSTICS
        let mut diagnostics = DIAGNOSTICS.lock().unwrap();
        diagnostics.push(msg.to_string());
    }
}

static DIAGNOSTICS: Mutex<Vec<String>> = Mutex::new(Vec::new());

struct MyCallbacks;

impl Callbacks for MyCallbacks {
    fn config(&mut self, config: &mut Config) {
        config.register_lints = Some(Box::new(|_sess, lint_store: &mut LintStore| {
            lint_store.register_late_pass(|_| Box::new(ExampleLateLintPass));
        }));
    }
}

#[test]
fn test_custom_lint() {
    let temp_path = PathBuf::from("temp.rs");
    let source_code = r#"
        fn main() {
            let x = 1;
            let y = 2;
            let z = x + y;
        }
    "#;
    write(&temp_path, source_code).expect("Failed to write temporary file");

    DIAGNOSTICS.lock().unwrap().clear();

    let args = vec![
        "rustc".to_string(),
        "--edition=2021".to_string(),
        temp_path.to_str().unwrap().to_string(),
    ];
    run_compiler(&args, &mut MyCallbacks);

    let diagnostics = DIAGNOSTICS.lock().unwrap();
    assert!(!diagnostics.is_empty(), "No diagnostics were collected");
    for diag in diagnostics.iter() {
        assert_eq!(diag, "test lint triggered");
        println!("{}", diag);
    }

    std::fs::remove_file(temp_path).ok();
}