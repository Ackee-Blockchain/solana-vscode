use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as TokioMutex;
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};

use crate::core::detector_config::DetectorConfig;
use crate::core::detectors::detector::DetectorWrapper;

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

    pub async fn analyze_file(
        &mut self,
        file_path: &Path,
        content: &str,
        detectors: &mut Vec<DetectorWrapper>,
        configs: &HashMap<String, DetectorConfig>,
    ) -> Vec<Diagnostic> {
        // Check cache first
        let cache = self.cache.lock().await;
        if let Some(cached) = cache.get(file_path) {
            return cached.clone();
        }
        drop(cache);

        let diagnostics = self
            .perform_analysis(file_path, content, detectors, configs)
            .await;

        let mut cache = self.cache.lock().await;
        cache.insert(file_path.to_path_buf(), diagnostics.clone());
        diagnostics
    }

    async fn perform_analysis(
        &self,
        file_path: &Path,
        content: &str,
        detectors: &mut Vec<DetectorWrapper>,
        configs: &HashMap<String, DetectorConfig>,
    ) -> Vec<Diagnostic> {
        let workspace_root = match &self.workspace_root {
            Some(root) => root,
            None => return vec![],
        };

        // First, run cargo check to get type information
        let type_info = match self.extract_type_info(workspace_root, file_path).await {
            Ok(info) => info,
            Err(e) => {
                log::warn!(
                    "Failed to extract type info for {}: {}",
                    file_path.display(),
                    e
                );
                return vec![];
            }
        };

        // Cache the type information
        let mut type_cache = self.type_info_cache.lock().await;
        type_cache.insert(file_path.to_path_buf(), type_info.clone());
        drop(type_cache);

        // Now run our detectors with the type information
        let mut all_diagnostics = Vec::new();

        for detector in detectors.iter_mut() {
            if let DetectorWrapper::Clippy(det) = detector {
                if configs.get(det.id()).map_or(true, |c| c.enabled) {
                    let context = ClippyAnalysisContext {
                        file_path: file_path.to_path_buf(),
                        content: content.to_string(),
                        type_info: type_info.clone(),
                        workspace_root: workspace_root.clone(),
                    };

                    let diagnostics = det.analyze_with_context(&context);
                    all_diagnostics.extend(diagnostics);
                }
            }
        }

        all_diagnostics
    }

    /// Extract type information using cargo check
    async fn extract_type_info(
        &self,
        workspace_root: &Path,
        file_path: &Path,
    ) -> Result<Vec<TypeInfo>> {
        let mut cmd = Command::new("cargo");
        cmd.current_dir(workspace_root)
            .args(&["check", "--message-format=json", "--quiet"]);

        // If we have a specific file, try to check just that crate
        if let Some(relative_path) = file_path.strip_prefix(workspace_root).ok() {
            if let Some(src_dir) = relative_path.parent() {
                if src_dir.join("Cargo.toml").exists() {
                    cmd.current_dir(workspace_root.join(src_dir));
                }
            }
        }

        let output = cmd.output().context("Failed to execute cargo check")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            log::warn!("Cargo check failed: {}", stderr);
            return Ok(vec![]);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut type_info = Vec::new();

        // extract type info
        todo!();

        Ok(type_info)
    }

    /// Get cached type information for a file
    pub async fn get_type_info(&self, file_path: &Path) -> Vec<TypeInfo> {
        let cache = self.type_info_cache.lock().await;
        cache.get(file_path).cloned().unwrap_or_default()
    }
}
