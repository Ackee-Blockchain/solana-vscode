use std::path::PathBuf;
use tower_lsp::lsp_types::Diagnostic;

/// Result of workspace scanning
/// Only Rust (.rs) files are scanned for security issues
#[derive(Debug, Default)]
pub struct ScanResult {
    pub rust_files: Vec<RustFileInfo>,
}

impl ScanResult {
    /// Get total number of issues found across all files
    pub fn total_issues(&self) -> usize {
        self.rust_files.iter().map(|f| f.diagnostics.len()).sum()
    }

    /// Get files with security issues
    pub fn files_with_issues(&self) -> Vec<&RustFileInfo> {
        self.rust_files
            .iter()
            .filter(|f| !f.diagnostics.is_empty())
            .collect()
    }

    /// Get Anchor program files
    pub fn anchor_program_files(&self) -> Vec<&RustFileInfo> {
        self.rust_files
            .iter()
            .filter(|f| f.is_anchor_program)
            .collect()
    }
}

/// Information about a scanned Rust file
/// Test files are excluded from scanning
#[derive(Debug)]
pub struct RustFileInfo {
    pub path: PathBuf,
    pub diagnostics: Vec<Diagnostic>,
    pub is_anchor_program: bool,
}
