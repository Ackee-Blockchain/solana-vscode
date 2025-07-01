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

        // Scan for Rust files
        self.scan_rust_files_with_client(root, detector_registry, &mut result, client)
            .await;

        // Scan for Anchor.toml files
        self.scan_anchor_config_files(root, &mut result).await;

        // Scan for Cargo.toml files
        self.scan_cargo_files(root, &mut result).await;

        info!(
            "Workspace scan completed. Found {} Rust files, {} Anchor configs, {} Cargo files",
            result.rust_files.len(),
            result.anchor_configs.len(),
            result.cargo_files.len()
        );

        result
    }

    /// Scan for Rust files and analyze them
    #[allow(dead_code)]
    async fn scan_rust_files(
        &self,
        root: &Path,
        detector_registry: &mut DetectorRegistry,
        result: &mut ScanResult,
    ) {
        self.scan_rust_files_with_client(root, detector_registry, result, None)
            .await;
    }

    /// Scan for Rust files and analyze them with optional progress notifications
    async fn scan_rust_files_with_client(
        &self,
        root: &Path,
        detector_registry: &mut DetectorRegistry,
        result: &mut ScanResult,
        _client: Option<&Client>,
    ) {
        if let Ok(entries) = self.walk_directory(root, &["rs"]) {
            for file_path in entries {
                if let Ok(content) = fs::read_to_string(&file_path) {
                    debug!("Analyzing Rust file: {:?}", file_path);

                    // Run security analysis
                    let diagnostics = detector_registry.analyze(&content, Some(&file_path));

                    if !diagnostics.is_empty() {
                        info!(
                            "Found {} issues in file: {:?}",
                            diagnostics.len(),
                            file_path
                        );
                    }

                    let is_anchor_program = self.is_anchor_program(&content);
                    let is_test_file = self.is_test_file(&file_path);

                    result.rust_files.push(RustFileInfo {
                        path: file_path,
                        diagnostics,
                        is_anchor_program,
                        is_test_file,
                    });
                } else {
                    warn!("Failed to read file: {:?}", file_path);
                }
            }
        }
    }

    /// Scan for Anchor.toml configuration files
    async fn scan_anchor_config_files(&self, root: &Path, result: &mut ScanResult) {
        if let Ok(entries) = self.find_files_by_name(root, "Anchor.toml") {
            for file_path in entries {
                if let Ok(content) = fs::read_to_string(&file_path) {
                    debug!("Found Anchor config: {:?}", file_path);
                    result.anchor_configs.push(AnchorConfigInfo {
                        path: file_path,
                        content,
                    });
                }
            }
        }
    }

    /// Scan for Cargo.toml files
    async fn scan_cargo_files(&self, root: &Path, result: &mut ScanResult) {
        if let Ok(entries) = self.find_files_by_name(root, "Cargo.toml") {
            for file_path in entries {
                if let Ok(content) = fs::read_to_string(&file_path) {
                    debug!("Found Cargo config: {:?}", file_path);
                    let is_workspace = content.contains("[workspace]");
                    result.cargo_files.push(CargoFileInfo {
                        path: file_path,
                        content,
                        is_workspace,
                    });
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
                // Skip common directories that shouldn't be scanned
                if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
                    if matches!(
                        dir_name,
                        "target" | "node_modules" | ".git" | ".vscode" | "out"
                    ) {
                        continue;
                    }
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

    /// Find files by exact name
    fn find_files_by_name(
        &self,
        dir: &Path,
        filename: &str,
    ) -> Result<Vec<PathBuf>, std::io::Error> {
        let mut files = Vec::new();
        self.find_files_by_name_recursive(dir, filename, &mut files)?;
        Ok(files)
    }

    /// Recursive helper for finding files by name
    #[allow(clippy::only_used_in_recursion)]
    fn find_files_by_name_recursive(
        &self,
        dir: &Path,
        filename: &str,
        files: &mut Vec<PathBuf>,
    ) -> Result<(), std::io::Error> {
        if !dir.is_dir() {
            return Ok(());
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // Skip common directories that shouldn't be scanned
                if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
                    if matches!(
                        dir_name,
                        "target" | "node_modules" | ".git" | ".vscode" | "out"
                    ) {
                        continue;
                    }
                }
                self.find_files_by_name_recursive(&path, filename, files)?;
            } else if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                if file_name == filename {
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

    /// Check if a file is a test file
    fn is_test_file(&self, path: &Path) -> bool {
        path.to_string_lossy().contains("test")
            || path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|name| name.contains("test"))
                .unwrap_or(false)
    }
}
