#![feature(rustc_private)]
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as TokioMutex;
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};

use clippy_utils::diagnostics::span_lint;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::Config;
use rustc_lint::{LateContext, LateLintPass, Lint, LintStore};
use rustc_session::Session;
use rustc_span::Span;

use crate::core::detectors::detector::ClippyAnalysisContext;

/// Information extracted from cargo check output
#[derive(Debug, Clone)]
pub struct TypeInfo {
    pub file_path: PathBuf,
    pub line: u32,
    pub column: u32,
    pub expr_type: String,
    pub span_start: u32,
    pub span_end: u32,
}

// static MY_LINT: &Lint = &Lint {
//     name: "my_lint",
//     default_level: rustc_lint::Level::Warn,
//     desc: "example lint for demonstration",
//     report_in_external_macro: false,
// };

#[derive(Copy, Clone)]
struct MyLint;

// impl<'tcx> LateLintPass<'tcx> for MyLint {
//     fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx rustc_hir::Expr<'_>) {
//         span_lint(cx, MY_LINT, expr.span, "found an expression!");
//     }
// }

/// Global diagnostics collector for rustc callbacks
static COLLECTED_DIAGNOSTICS: Mutex<Vec<Diagnostic>> = Mutex::new(Vec::new());

/// Rustc callbacks implementation
struct ClippyCallbacks;

impl Callbacks for ClippyCallbacks {
    fn config(&mut self, config: &mut rustc_interface::Config) {
        config.register_lints = Some(Box::new(|_sess, lint_store: &mut LintStore| {
            // lint_store.register_late_pass(|_| Box::new(MyLint));
        }));
    }
}

pub struct ClippyAnalyzer {
    cache: Arc<TokioMutex<HashMap<PathBuf, Vec<Diagnostic>>>>,
    workspace_root: Option<PathBuf>,
    type_info_cache: Arc<TokioMutex<HashMap<PathBuf, Vec<TypeInfo>>>>,
}

impl ClippyAnalyzer {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(TokioMutex::new(HashMap::new())),
            workspace_root: None,
            type_info_cache: Arc::new(TokioMutex::new(HashMap::new())),
        }
    }

    pub fn set_workspace_root(&mut self, root: PathBuf) {
        self.workspace_root = Some(root);
    }

    /// Analyze code using rustc driver with clippy-style lints
    pub fn analyze_with_clippy(&self, context: &ClippyAnalysisContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let file_path = context.file_path.clone();
        let content = context.source_code.clone();


            // Clear previous diagnostics
            // DIAGNOSTICS.lock().unwrap().clear();

            // Run rustc with custom callbacks
            let args = vec![
                "rustc".to_string(),
                "--edition=2021".to_string(),
                file_path.to_str().unwrap().to_string(),
            ];
            // rustc_driver::RunCompiler::new(&args, &mut MyCallbacks).run().ok();

            // Collect diagnostics
            // let mut lint_diagnostics = DIAGNOSTICS.lock().unwrap().drain(..).collect::<Vec<_>>();
            // if let Some(severity_override) = config.severity_override {
            //     for diagnostic in &mut lint_diagnostics {
            //         diagnostic.severity = Some(severity_override.clone());
            //     }
            // }
            // diagnostics.extend(lint_diagnostics);
        

        diagnostics
    }

    /// Convert rustc diagnostics to LSP diagnostics
    fn convert_rustc_diagnostics(&self, rustc_diagnostics: &[Diagnostic]) -> Vec<Diagnostic> {
        rustc_diagnostics.iter().cloned().collect()
    }

    /// Create a simple example diagnostic for demonstration
    fn create_example_diagnostic(&self, line_number: usize, line: &str) -> Diagnostic {
        Diagnostic {
            range: Range::new(
                Position::new(line_number as u32, 0),
                Position::new(line_number as u32, line.len() as u32),
            ),
            severity: Some(DiagnosticSeverity::WARNING),
            code: Some(tower_lsp::lsp_types::NumberOrString::String(
                "CLIPPY_ANALYZER_EXAMPLE".to_string(),
            )),
            code_description: None,
            source: Some("clippy-analyzer".to_string()),
            message: format!("Example clippy analysis result: {}", line.trim()),
            related_information: None,
            tags: None,
            data: None,
        }
    }

    /// Clear cache for a specific file
    pub async fn clear_cache(&self, file_path: &PathBuf) {
        let mut cache = self.cache.lock().await;
        cache.remove(file_path);
    }

    /// Clear all cache
    pub async fn clear_all_cache(&self) {
        let mut cache = self.cache.lock().await;
        cache.clear();
    }
}
