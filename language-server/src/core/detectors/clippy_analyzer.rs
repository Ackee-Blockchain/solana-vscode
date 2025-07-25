use super::detector::{ClippyAnalysisContext, ClippyDetector, CompilationResult};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::Mutex;
use tower_lsp::lsp_types::Diagnostic;

/// Manager for clippy-style analysis with caching and background processing
pub struct ClippyAnalyzer {
    /// Cache of analysis results
    analysis_cache: Arc<Mutex<HashMap<PathBuf, CachedAnalysis>>>,
    /// Registered clippy detectors
    pub detectors: Vec<Box<dyn ClippyDetector>>,
    /// Workspace root for context
    workspace_root: Option<PathBuf>,
}

/// Cached analysis result with timestamp
#[derive(Debug, Clone)]
struct CachedAnalysis {
    diagnostics: Vec<Diagnostic>,
    timestamp: std::time::SystemTime,
    content_hash: u64,
}

impl ClippyAnalyzer {
    pub fn new() -> Self {
        Self {
            analysis_cache: Arc::new(Mutex::new(HashMap::new())),
            detectors: Vec::new(),
            workspace_root: None,
        }
    }

    /// Register a clippy-style detector
    pub fn register_detector<D: ClippyDetector + 'static>(&mut self, detector: D) {
        self.detectors.push(Box::new(detector));
    }

    /// Set the workspace root for compilation context
    pub fn set_workspace_root(&mut self, root: PathBuf) {
        self.workspace_root = Some(root);
    }

    /// Analyze a file with clippy-style detectors
    pub async fn analyze_file(&mut self, file_path: &Path, content: &str) -> Vec<Diagnostic> {
        // Check cache first
        let content_hash = self.calculate_content_hash(content);
        if let Some(cached) = self.get_cached_analysis(file_path, content_hash).await {
            return cached;
        }

        // Perform fresh analysis
        let diagnostics = self.perform_analysis(file_path, content).await;

        // Cache the results
        self.cache_analysis(file_path, content_hash, diagnostics.clone())
            .await;

        diagnostics
    }

    /// Perform the actual clippy-style analysis
    async fn perform_analysis(&mut self, file_path: &Path, content: &str) -> Vec<Diagnostic> {
        let mut all_diagnostics = Vec::new();

        // Create compilation context
        let context = match self.create_analysis_context(file_path, content).await {
            Ok(ctx) => ctx,
            Err(e) => {
                log::warn!(
                    "Failed to create analysis context for {:?}: {}",
                    file_path,
                    e
                );
                return all_diagnostics;
            }
        };

        // Run all clippy detectors
        for detector in &mut self.detectors {
            let diagnostics = detector.analyze_with_context(&context);
            all_diagnostics.extend(diagnostics);
        }

        all_diagnostics
    }

    /// Create analysis context by setting up a temporary compilation environment
    async fn create_analysis_context(
        &self,
        file_path: &Path,
        content: &str,
    ) -> Result<ClippyAnalysisContext, Box<dyn std::error::Error + Send + Sync>> {
        // Create temporary directory for compilation
        let temp_dir = TempDir::new()?;
        let temp_path = temp_dir.path().to_path_buf();

        // Create a minimal Cargo project structure
        self.setup_temp_project(&temp_path, file_path, content)
            .await?;

        // Attempt compilation to get HIR and type information
        let compilation_result = self.compile_for_analysis(&temp_path).await?;

        Ok(ClippyAnalysisContext {
            file_path: file_path.to_path_buf(),
            source_code: content.to_string(),
            compilation_result,
        })
    }

    /// Setup a temporary Cargo project for analysis
    async fn setup_temp_project(
        &self,
        temp_path: &Path,
        original_file: &Path,
        content: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Create Cargo.toml
        let cargo_toml = r#"[package]
name = "temp_analysis"
version = "0.1.0"
edition = "2021"

[dependencies]
anchor-lang = "0.30"
solana-program = "1.18"
"#;

        tokio::fs::write(temp_path.join("Cargo.toml"), cargo_toml).await?;

        // Create src directory
        let src_dir = temp_path.join("src");
        tokio::fs::create_dir_all(&src_dir).await?;

        // Write the source file
        let target_file = if original_file.file_name().unwrap_or_default() == "lib.rs" {
            src_dir.join("lib.rs")
        } else {
            // For other files, create a lib.rs that includes the module
            let lib_content = format!(
                "pub mod {};\n",
                original_file.file_stem().unwrap().to_string_lossy()
            );
            tokio::fs::write(src_dir.join("lib.rs"), lib_content).await?;
            src_dir.join(original_file.file_name().unwrap())
        };

        tokio::fs::write(target_file, content).await?;

        Ok(())
    }

    /// Compile the temporary project to get analysis information
    async fn compile_for_analysis(
        &self,
        temp_path: &Path,
    ) -> Result<CompilationResult, Box<dyn std::error::Error + Send + Sync>> {
        let output = Command::new("cargo")
            .args(&["check", "--message-format=json"])
            .current_dir(temp_path)
            .output()?;

        let success = output.status.success();

        // For now, we'll do a simple compilation check
        // In a full implementation, you'd integrate with rustc APIs here
        Ok(CompilationResult {
            success,
            hir_available: success,
            type_info_available: success,
            temp_dir: Some(temp_path.to_path_buf()),
        })
    }

    /// Calculate hash of content for caching
    fn calculate_content_hash(&self, content: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        hasher.finish()
    }

    /// Get cached analysis if available and fresh
    async fn get_cached_analysis(
        &self,
        file_path: &Path,
        content_hash: u64,
    ) -> Option<Vec<Diagnostic>> {
        let cache = self.analysis_cache.lock().await;
        if let Some(cached) = cache.get(file_path) {
            if cached.content_hash == content_hash {
                // Check if cache is still fresh (e.g., less than 5 minutes old)
                if let Ok(elapsed) = cached.timestamp.elapsed() {
                    if elapsed.as_secs() < 300 {
                        return Some(cached.diagnostics.clone());
                    }
                }
            }
        }
        None
    }

    /// Cache analysis results
    async fn cache_analysis(
        &self,
        file_path: &Path,
        content_hash: u64,
        diagnostics: Vec<Diagnostic>,
    ) {
        let mut cache = self.analysis_cache.lock().await;
        cache.insert(
            file_path.to_path_buf(),
            CachedAnalysis {
                diagnostics,
                timestamp: std::time::SystemTime::now(),
                content_hash,
            },
        );
    }

    /// Clear cache for a specific file
    pub async fn invalidate_cache(&self, file_path: &Path) {
        let mut cache = self.analysis_cache.lock().await;
        cache.remove(file_path);
    }

    /// Clear all cached analyses
    pub async fn clear_cache(&self) {
        let mut cache = self.analysis_cache.lock().await;
        cache.clear();
    }
}
impl Default for ClippyAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}
