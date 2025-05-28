use crate::core::ScanResult;
use serde::{Deserialize, Serialize};

/// Custom notification for sending scan results to the extension
#[derive(Debug)]
pub enum ScanCompleteNotification {}

impl tower_lsp::lsp_types::notification::Notification for ScanCompleteNotification {
    type Params = ScanSummary;
    const METHOD: &'static str = "solana/scanComplete";
}

/// Custom notification for sending scan progress updates
#[derive(Debug)]
#[allow(dead_code)]
pub enum ScanProgressNotification {}

impl tower_lsp::lsp_types::notification::Notification for ScanProgressNotification {
    type Params = ScanProgress;
    const METHOD: &'static str = "solana/scanProgress";
}

/// Custom notification for sending file analysis results
#[derive(Debug)]
#[allow(dead_code)]
pub enum FileAnalysisNotification {}

impl tower_lsp::lsp_types::notification::Notification for FileAnalysisNotification {
    type Params = FileAnalysisResult;
    const METHOD: &'static str = "solana/fileAnalysis";
}

/// Summary of scan results to send to the extension
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanSummary {
    pub total_rust_files: usize,
    pub anchor_program_files: usize,
    pub files_with_issues: usize,
    pub total_issues: usize,
    pub anchor_configs: usize,
    pub cargo_files: usize,
    pub issues_by_file: Vec<FileIssueInfo>,
}

impl ScanSummary {
    pub fn from_scan_result(scan_result: &ScanResult) -> Self {
        let issues_by_file = scan_result
            .files_with_issues()
            .iter()
            .map(|file_info| FileIssueInfo {
                path: file_info.path.to_string_lossy().to_string(),
                issue_count: file_info.diagnostics.len(),
                is_anchor_program: file_info.is_anchor_program,
                is_test_file: file_info.is_test_file,
            })
            .collect();

        Self {
            total_rust_files: scan_result.rust_files.len(),
            anchor_program_files: scan_result.anchor_program_files().len(),
            files_with_issues: scan_result.files_with_issues().len(),
            total_issues: scan_result.total_issues(),
            anchor_configs: scan_result.anchor_configs.len(),
            cargo_files: scan_result.cargo_files.len(),
            issues_by_file,
        }
    }
}

/// Information about a file with security issues
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileIssueInfo {
    pub path: String,
    pub issue_count: usize,
    pub is_anchor_program: bool,
    pub is_test_file: bool,
}

/// Progress information for ongoing scans
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanProgress {
    pub current_file: String,
    pub files_processed: usize,
    pub total_files: usize,
    pub issues_found_so_far: usize,
}

/// Result of analyzing a single file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileAnalysisResult {
    pub path: String,
    pub issue_count: usize,
    pub is_anchor_program: bool,
    pub is_test_file: bool,
    pub analysis_time_ms: u64,
}
