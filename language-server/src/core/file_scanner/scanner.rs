use crate::core::{DetectorRegistry, file_scanner::types::*};
use log::{debug, info, warn};
use std::fs;
use std::path::{Path, PathBuf};
use tower_lsp::Client;

/// File scanner for analyzing workspace files on startup
#[derive(Default, Debug)]
pub struct FileScanner {
    workspace_root: Option<PathBuf>,
}

impl FileScanner {
    /// Set the workspace root directory
    pub fn set_workspace_root(&mut self, root: PathBuf) {
        self.workspace_root = Some(root.clone());
        info!("Workspace root set to: {:?}", root);
    }

    /// Scan all relevant files in the workspace
    pub async fn scan_workspace(&self, detector_registry: &mut DetectorRegistry) -> ScanResult {
        self.scan_workspace_with_client(detector_registry, None)
            .await
    }

    /// Scan all relevant files in the workspace with optional progress notifications
    pub async fn scan_workspace_with_client(
        &self,
        detector_registry: &mut DetectorRegistry,
        client: Option<&Client>,
    ) -> ScanResult {
        let Some(root) = &self.workspace_root else {
            warn!("No workspace root set, skipping file scan");
            return ScanResult::default();
        };

        info!("Starting workspace file scan from: {:?}", root);

        let mut result = ScanResult::default();

        // Scan for Rust files and run security analysis
        // Only .rs files in the workspace are analyzed (excludes external dependencies)
        self.scan_rust_files_with_client(root, detector_registry, &mut result, client)
            .await;

        info!(
            "Workspace scan completed. Found {} Rust files ({} Anchor programs, {} with issues)",
            result.rust_files.len(),
            result.anchor_program_files().len(),
            result.files_with_issues().len()
        );

        result
    }

    /// Scan for Rust files and analyze them with optional progress notifications
    /// Only scans .rs files within the workspace, excluding external dependencies
    async fn scan_rust_files_with_client(
        &self,
        root: &Path,
        detector_registry: &mut DetectorRegistry,
        result: &mut ScanResult,
        _client: Option<&Client>,
    ) {
        // Only scan .rs files (Rust source files), excluding test files
        if let Ok(entries) = self.walk_directory(root, &["rs"]) {
            for file_path in entries {
                // Skip dedicated test files (in tests/ directories or with test in filename)
                if self.is_test_file(&file_path) {
                    debug!("Skipping test file: {:?}", file_path);
                    continue;
                }

                if let Ok(content) = fs::read_to_string(&file_path) {
                    debug!("Analyzing Rust file: {:?}", file_path);

                    // Run security analysis on Rust source code
                    // Detectors will naturally skip test modules (#[cfg(test)]) during AST analysis
                    let diagnostics = detector_registry.analyze(&content, Some(&file_path));

                    if !diagnostics.is_empty() {
                        info!(
                            "Found {} issues in file: {:?}",
                            diagnostics.len(),
                            file_path
                        );
                    }

                    let is_anchor_program = self.is_anchor_program(&content);

                    result.rust_files.push(RustFileInfo {
                        path: file_path,
                        diagnostics,
                        is_anchor_program,
                    });
                } else {
                    warn!("Failed to read file: {:?}", file_path);
                }
            }
        }
    }

    /// Walk directory recursively and find files with specific extensions
    fn walk_directory(
        &self,
        dir: &Path,
        extensions: &[&str],
    ) -> Result<Vec<PathBuf>, std::io::Error> {
        let mut files = Vec::new();
        self.walk_directory_recursive(dir, extensions, &mut files)?;
        Ok(files)
    }

    /// Recursive helper for walking directories
    #[allow(clippy::only_used_in_recursion)]
    fn walk_directory_recursive(
        &self,
        dir: &Path,
        extensions: &[&str],
        files: &mut Vec<PathBuf>,
    ) -> Result<(), std::io::Error> {
        if !dir.is_dir() {
            return Ok(());
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // Skip directories containing external dependencies and build artifacts
                if self.should_skip_directory(&path) {
                    continue;
                }
                self.walk_directory_recursive(&path, extensions, files)?;
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if extensions.contains(&ext) {
                    files.push(path);
                }
            }
        }

        Ok(())
    }

    /// Check if a Rust file contains Anchor program code
    fn is_anchor_program(&self, content: &str) -> bool {
        content.contains("anchor_lang")
            || content.contains("anchor_spl")
            || content.contains("#[program]")
            || content.contains("#[derive(Accounts)]")
    }

    /// Check if a file is a dedicated test file based on path
    /// Files with test modules inside production files will still be analyzed
    /// (detectors will naturally skip test modules during AST analysis)
    fn is_test_file(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();

        // Check if in tests/ directory (dedicated test files)
        if path_str.contains("/tests/") || path_str.contains("\\tests\\") {
            return true;
        }

        // Check if filename indicates it's a test file
        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
            // Skip files that are clearly test files (not lib.rs with test modules)
            if file_name.starts_with("test_") || file_name.ends_with("_test.rs") {
                return true;
            }
        }

        false
    }

    /// Determine if a directory should be skipped during scanning
    /// This filters out external dependencies, build artifacts, and IDE folders
    fn should_skip_directory(&self, path: &Path) -> bool {
        let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) else {
            return false;
        };

        // Skip hidden directories (except .anchor which we already handle separately)
        if dir_name.starts_with('.') && dir_name != ".anchor" {
            return true;
        }

        // Skip known directories containing external dependencies or build artifacts
        matches!(
            dir_name,
            // Rust build artifacts and dependencies
            "target" | "debug" | "release" | "deps" | "build" | "incremental" |
            // Test directories (skip all test code)
            "tests" | "test" | "trident-tests" |
            // JavaScript/TypeScript dependencies and build outputs
            "node_modules" | "dist" | "out" | "coverage" | ".nyc_output" |
            // Version control
            ".git" | ".svn" | ".hg" |
            // IDE and editor folders
            ".vscode" | ".idea" | ".vs" | ".fleet" |
            // Anchor framework folders
            ".anchor" |
            // Other common build/cache directories
            "tmp" | "temp" | ".cache" | ".parcel-cache" | ".next" | ".nuxt" |
            ".vuepress" | ".docusaurus" | ".gradle" | ".maven" |
            // Documentation build outputs
            "doc" | "docs-build" | "_book" | "_site" |
            // Dependency management artifacts
            "vendor" | "bower_components" | "jspm_packages" |
            // Test coverage and reports
            "htmlcov" | "test-results" | "test-reports" | ".pytest_cache" | ".tox"
        )
    }
}
