use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as TokioMutex;
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};

use crate::core::detector_config::DetectorConfig;
use crate::core::detectors::detector::{ClippyAnalysisContext, DetectorWrapper};

// Callbacks for rustc_driver
struct AnalyzerCallbacks {
    detectors: Arc<Mutex<Vec<DetectorWrapper>>>,
    configs: HashMap<String, DetectorConfig>,
    diagnostics: Arc<Mutex<Vec<Diagnostic>>>,
}

impl Callbacks for AnalyzerCallbacks {
    fn config(&mut self, config: &mut rustc_interface::Config) {
        config.opts.edition = Edition::Edition2021;
    }

    fn after_analysis<'tcx>(
        &mut self,
        _compiler: &Compiler,
        queries: &'tcx Queries<'tcx>,
    ) -> Compilation {
        queries.global_ctxt().unwrap().enter(|tcx| {
            let mut lint_store = LintStore::new();

            let detectors = self.detectors.lock().unwrap();
            for detector in detectors.iter() {
                if let DetectorWrapper::Clippy(det) = detector {
                    if self.configs.get(det.id()).map_or(true, |c| c.enabled) {
                        lint_store.register_late_pass(|_| Box::new(det.clone()));
                    }
                }
            }
            rustc_lint::late::check_crate(tcx, &lint_store);

            let mut all_diagnostics = Vec::new();
            for detector in detectors.iter() {
                if let DetectorWrapper::Clippy(det) = detector {
                    all_diagnostics.extend(det.get_diagnostics());
                }
            }
            *self.diagnostics.lock().unwrap() = all_diagnostics;
        });
        Compilation::Stop
    }
}

/// ClippyAnalyzer using rustc_driver
pub struct ClippyAnalyzer {
    cache: Arc<TokioMutex<HashMap<PathBuf, Vec<Diagnostic>>>>,
    workspace_root: Option<PathBuf>,
}

impl ClippyAnalyzer {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(TokioMutex::new(HashMap::new())),
            workspace_root: None,
        }
    }

    pub fn set_workspace_root(&mut self, root: PathBuf) {
        self.workspace_root = Some(root);
    }

    pub async fn analyze_file(
        &mut self,
        file_path: &Path,
        _content: &str, // May not need if compiling whole crate
        detectors: &mut Vec<DetectorWrapper>,
        configs: &HashMap<String, DetectorConfig>,
    ) -> Vec<Diagnostic> {
        // Simplified caching; improve with content hash
        let cache = self.cache.lock().await;
        if let Some(cached) = cache.get(file_path) {
            return cached.clone();
        }
        drop(cache);

        let diagnostics = self.perform_analysis(file_path, detectors, configs).await;

        let mut cache = self.cache.lock().await;
        cache.insert(file_path.to_path_buf(), diagnostics.clone());
        diagnostics
    }

    async fn perform_analysis(
        &self,
        file_path: &Path,
        detectors: &mut Vec<DetectorWrapper>,
        configs: &HashMap<String, DetectorConfig>,
    ) -> Vec<Diagnostic> {
        let crate_root = self.workspace_root.as_ref().unwrap(); // Assume set
        let args = Self::build_compiler_args(crate_root, file_path);

        let diagnostics_arc = Arc::new(Mutex::new(Vec::new()));
        let detectors_arc = Arc::new(Mutex::new(detectors.clone()));
        let mut callbacks = AnalyzerCallbacks {
            detectors: detectors_arc,
            configs: configs.clone(),
            diagnostics: diagnostics_arc.clone(),
        };

        if RunCompiler::new(&args, &mut callbacks).run().is_err() {
            return vec![];
        }

        diagnostics_arc.lock().unwrap().clone()
    }

    fn build_compiler_args(crate_root: &Path, file_path: &Path) -> Vec<String> {
        // Build args: sysroot, externs, target, etc.
        vec![
            "rustc".to_string(),
            file_path.to_str().unwrap().to_string(),
            "--crate-type".to_string(),
            "lib".to_string(),
            "--target".to_string(),
            "sbf-unknown-unknown".to_string(),
            // Add more from Cargo check or similar
        ]
    }
}
