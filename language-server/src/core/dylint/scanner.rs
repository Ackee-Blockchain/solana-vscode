use log::{debug, info, warn};
use std::fs;
use std::path::{Path, PathBuf};

/// Information about a dylint detector crate
#[derive(Debug, Clone)]
pub struct DylintDetectorInfo {
    pub crate_path: PathBuf,
    pub crate_name: String,
    pub cargo_toml_path: PathBuf,
}

/// Scanner for finding dylint detector crates in the workspace
#[derive(Debug)]
pub struct DylintDetectorScanner {
    workspace_root: Option<PathBuf>,
}

impl DylintDetectorScanner {
    pub fn new() -> Self {
        Self {
            workspace_root: None,
        }
    }

    pub fn set_workspace_root(&mut self, root: PathBuf) {
        self.workspace_root = Some(root);
    }

    /// Scan for dylint detector crates in the workspace
    pub fn scan_detectors(&self) -> Vec<DylintDetectorInfo> {
        let Some(root) = &self.workspace_root else {
            warn!("No workspace root set, cannot scan for dylint detectors");
            return Vec::new();
        };

        info!(
            "[Workspace Dylint] Scanning for dylint detector crates in: {:?}",
            root
        );
        let mut detectors = Vec::new();

        // Look for Cargo.toml files that might be dylint detectors
        if let Ok(cargo_files) = self.find_cargo_toml_files(root) {
            for cargo_toml in cargo_files {
                if let Some(detector_info) = self.check_if_dylint_detector(&cargo_toml) {
                    info!(
                        "Found dylint detector: {} at {:?}",
                        detector_info.crate_name, detector_info.crate_path
                    );
                    detectors.push(detector_info);
                }
            }
        }

        info!(
            "[Workspace Dylint] Found {} dylint detector(s)",
            detectors.len()
        );
        detectors
    }

    /// Find all Cargo.toml files in the workspace
    fn find_cargo_toml_files(&self, root: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
        let mut files = Vec::new();
        self.find_cargo_toml_recursive(root, &mut files)?;
        Ok(files)
    }

    fn find_cargo_toml_recursive(
        &self,
        dir: &Path,
        files: &mut Vec<PathBuf>,
    ) -> Result<(), std::io::Error> {
        if !dir.is_dir() {
            return Ok(());
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // Skip common directories
                if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
                    if matches!(
                        dir_name,
                        "target" | "node_modules" | ".git" | ".vscode" | "out" | ".anchor"
                    ) {
                        continue;
                    }
                }
                self.find_cargo_toml_recursive(&path, files)?;
            } else if path.file_name().and_then(|n| n.to_str()) == Some("Cargo.toml") {
                files.push(path);
            }
        }

        Ok(())
    }

    /// Check if a Cargo.toml represents a dylint detector
    fn check_if_dylint_detector(&self, cargo_toml: &Path) -> Option<DylintDetectorInfo> {
        let content = match fs::read_to_string(cargo_toml) {
            Ok(c) => c,
            Err(e) => {
                debug!("Failed to read {:?}: {}", cargo_toml, e);
                return None;
            }
        };

        // Check if it's a dylint detector:
        // 1. Has dylint as a dependency
        // 2. Has [lib] section with crate-type = ["dylib"]
        // 3. Or has proc-macro = true (some dylint detectors use proc macros)
        let is_dylint = content.contains("dylint")
            && (content.contains("crate-type = [\"dylib\"]")
                || content.contains("crate-type = [\"cdylib\"]")
                || content.contains("proc-macro = true"));

        if !is_dylint {
            return None;
        }

        // Extract crate name from Cargo.toml
        let crate_name = self.extract_crate_name(&content)?;
        let crate_path = cargo_toml.parent()?.to_path_buf();

        Some(DylintDetectorInfo {
            crate_path,
            crate_name,
            cargo_toml_path: cargo_toml.to_path_buf(),
        })
    }

    /// Extract crate name from Cargo.toml content
    fn extract_crate_name(&self, content: &str) -> Option<String> {
        // Look for [package] name = "..."
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("name =") {
                if let Some(name) = line.strip_prefix("name =") {
                    let name = name.trim();
                    // Remove quotes
                    let name = name.trim_matches('"').trim_matches('\'');
                    return Some(name.to_string());
                }
            }
        }
        None
    }
}

impl Default for DylintDetectorScanner {
    fn default() -> Self {
        Self::new()
    }
}
