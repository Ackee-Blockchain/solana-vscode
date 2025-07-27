use crate::core::{
    ClippyUncheckedArithmeticDetector, DetectorInfo, DetectorRegistry, DetectorRegistryBuilder,
    FileScanner, ImmutableAccountMutatedDetector, InstructionAttributeInvalidDetector,
    InstructionAttributeUnusedDetector, ManualLamportsZeroingDetector, MissingCheckCommentDetector,
    MissingInitspaceDetector, MissingSignerDetector, ScanCompleteNotification, ScanResult,
    ScanSummary, SysvarAccountDetector, UnsafeMathDetector,
};
use log::info;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_lsp::{
    Client, LanguageServer,
    jsonrpc::Result as JsonRpcResult,
    lsp_types::{
        DidChangeTextDocumentParams, DidOpenTextDocumentParams, InitializeParams, InitializeResult,
        PositionEncodingKind, ServerCapabilities, ServerInfo, TextDocumentItem,
        TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
    },
};

#[derive(Debug, Clone)]
pub struct Backend {
    client: Client,
    detector_registry: Arc<Mutex<DetectorRegistry>>,
    file_scanner: Arc<Mutex<FileScanner>>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(
        &self,
        params: InitializeParams,
    ) -> Result<InitializeResult, tower_lsp::jsonrpc::Error> {
        // Set up workspace root if provided
        if let Some(workspace_folders) = params.workspace_folders {
            if let Some(folder) = workspace_folders.first() {
                if let Ok(path) = folder.uri.to_file_path() {
                    let mut scanner = self.file_scanner.lock().await;
                    scanner.set_workspace_root(path.clone());

                    // Set workspace root for clippy analysis
                    let mut registry = self.detector_registry.lock().await;
                    registry.set_workspace_root(path);

                    // Perform initial workspace scan
                    info!("Performing initial workspace scan...");
                    let scan_result = scanner.scan_workspace(&mut registry).await;

                    // Log scan results
                    info!("Initial scan completed:");
                    info!("  - {} Rust files found", scan_result.rust_files.len());
                    info!(
                        "  - {} Anchor programs found",
                        scan_result.anchor_program_files().len()
                    );
                    info!(
                        "  - {} files with security issues",
                        scan_result.files_with_issues().len()
                    );
                    info!(
                        "  - {} total security issues found",
                        scan_result.total_issues()
                    );
                    info!(
                        "  - {} Anchor.toml files found",
                        scan_result.anchor_configs.len()
                    );
                    info!(
                        "  - {} Cargo.toml files found",
                        scan_result.cargo_files.len()
                    );

                    // Optionally publish diagnostics for files with issues
                    for file_info in scan_result.files_with_issues() {
                        if let Ok(uri) = tower_lsp::lsp_types::Url::from_file_path(&file_info.path)
                        {
                            self.client
                                .publish_diagnostics(uri, file_info.diagnostics.clone(), None)
                                .await;
                        }
                    }

                    // Send scan results to extension
                    let scan_summary = ScanSummary::from_scan_result(&scan_result);
                    self.client
                        .send_notification::<ScanCompleteNotification>(scan_summary)
                        .await;
                }
            }
        } else if let Some(root_uri) = params.root_uri {
            if let Ok(path) = root_uri.to_file_path() {
                let mut scanner = self.file_scanner.lock().await;
                scanner.set_workspace_root(path.clone());

                // Set workspace root for clippy analysis
                let mut registry = self.detector_registry.lock().await;
                registry.set_workspace_root(path);

                // Perform initial workspace scan
                info!("Performing initial workspace scan...");
                let scan_result = scanner.scan_workspace(&mut registry).await;

                // Log scan results
                info!("Initial scan completed:");
                info!("  - {} Rust files found", scan_result.rust_files.len());
                info!(
                    "  - {} Anchor programs found",
                    scan_result.anchor_program_files().len()
                );
                info!(
                    "  - {} files with security issues",
                    scan_result.files_with_issues().len()
                );
                info!(
                    "  - {} total security issues found",
                    scan_result.total_issues()
                );
                info!(
                    "  - {} Anchor.toml files found",
                    scan_result.anchor_configs.len()
                );
                info!(
                    "  - {} Cargo.toml files found",
                    scan_result.cargo_files.len()
                );

                // Optionally publish diagnostics for files with issues
                for file_info in scan_result.files_with_issues() {
                    if let Ok(uri) = tower_lsp::lsp_types::Url::from_file_path(&file_info.path) {
                        self.client
                            .publish_diagnostics(uri, file_info.diagnostics.clone(), None)
                            .await;
                    }
                }

                // Send scan results to extension
                let scan_summary = ScanSummary::from_scan_result(&scan_result);
                self.client
                    .send_notification::<ScanCompleteNotification>(scan_summary)
                    .await;
            }
        }

        let result = InitializeResult {
            server_info: Some(ServerInfo {
                name: env!("CARGO_PKG_NAME").to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
            capabilities: ServerCapabilities {
                position_encoding: Some(PositionEncodingKind::UTF16),
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        ..Default::default()
                    },
                )),
                execute_command_provider: Some(tower_lsp::lsp_types::ExecuteCommandOptions {
                    commands: vec!["workspace.scan".to_string()],
                    work_done_progress_options: Default::default(),
                }),
                ..Default::default()
            },
        };
        Ok(result)
    }
    async fn shutdown(&self) -> Result<(), tower_lsp::jsonrpc::Error> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.on_change(params.text_document).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let text_document = TextDocumentItem {
            uri: params.text_document.uri,
            language_id: "rust".to_string(),
            version: params.text_document.version,
            text: params.content_changes[0].text.clone(),
        };
        self.on_change(text_document).await;
    }

    async fn execute_command(
        &self,
        params: tower_lsp::lsp_types::ExecuteCommandParams,
    ) -> JsonRpcResult<Option<serde_json::Value>> {
        match params.command.as_str() {
            "workspace.scan" => {
                info!("Manual workspace scan requested");

                // Perform workspace scan
                let scan_result = {
                    let scanner = self.file_scanner.lock().await;
                    let mut registry = self.detector_registry.lock().await;
                    scanner.scan_workspace(&mut registry).await
                };

                // Log scan results
                info!("Manual scan completed:");
                info!("  - {} Rust files found", scan_result.rust_files.len());
                info!(
                    "  - {} Anchor programs found",
                    scan_result.anchor_program_files().len()
                );
                info!(
                    "  - {} files with security issues",
                    scan_result.files_with_issues().len()
                );
                info!(
                    "  - {} total security issues found",
                    scan_result.total_issues()
                );

                // Publish diagnostics for files with issues
                for file_info in scan_result.files_with_issues() {
                    if let Ok(uri) = tower_lsp::lsp_types::Url::from_file_path(&file_info.path) {
                        self.client
                            .publish_diagnostics(uri, file_info.diagnostics.clone(), None)
                            .await;
                    }
                }

                // Send scan results to extension
                let scan_summary = ScanSummary::from_scan_result(&scan_result);
                self.client
                    .send_notification::<ScanCompleteNotification>(scan_summary)
                    .await;

                Ok(Some(serde_json::json!({
                    "success": true,
                    "total_files": scan_result.rust_files.len(),
                    "total_issues": scan_result.total_issues()
                })))
            }
            _ => Ok(None),
        }
    }
}

impl Backend {
    pub fn new(client: Client) -> Backend {
        Backend {
            client,
            detector_registry: Arc::new(Mutex::new(create_default_registry())),
            file_scanner: Arc::new(Mutex::new(FileScanner::default())),
        }
    }

    async fn on_change(&self, params: TextDocumentItem) {
        // Run immediate syn-based analysis first (fast)
        let immediate_diagnostics = {
            let mut registry = self.detector_registry.lock().await;
            let file_path = params.uri.to_file_path().ok();
            registry.analyze_immediate(&params.text, file_path.as_ref())
        };

        // Publish immediate diagnostics
        self.client
            .publish_diagnostics(
                params.uri.clone(),
                immediate_diagnostics,
                Some(params.version),
            )
            .await;

        // // Schedule background clippy analysis
        // let client = self.client.clone();
        // let registry = Arc::clone(&self.detector_registry);

        // tokio::spawn(async move {
        //     if let Ok(file_path) = params.uri.to_file_path() {
        //         let comprehensive_diagnostics = {
        //             let mut registry = registry.lock().await;
        //             registry.analyze_comprehensive(&file_path, &params.text).await
        //         };

        //         // Merge with immediate diagnostics and republish
        //         let all_diagnostics = {
        //             let mut registry = registry.lock().await;
        //             let mut diagnostics = registry.analyze_immediate(&params.text, Some(&file_path));
        //             diagnostics.extend(comprehensive_diagnostics);
        //             diagnostics
        //         };

        //         client
        //             .publish_diagnostics(params.uri.clone(), all_diagnostics, Some(params.version))
        //             .await;
        //     }
        // });
    }

    /// Get information about all registered detectors
    #[allow(dead_code)]
    pub async fn list_detectors(&self) -> Vec<DetectorInfo> {
        let registry = self.detector_registry.lock().await;
        registry.list_detectors()
    }

    /// Enable or disable a specific detector
    #[allow(dead_code)]
    pub async fn set_detector_enabled(&self, detector_id: &str, enabled: bool) {
        let mut registry = self.detector_registry.lock().await;
        if enabled {
            registry.enable(detector_id);
        } else {
            registry.disable(detector_id);
        }
    }

    /// Get detector statistics
    #[allow(dead_code)]
    pub async fn get_detector_stats(&self) -> DetectorStats {
        let registry = self.detector_registry.lock().await;
        DetectorStats {
            total_detectors: registry.count(),
            enabled_detectors: registry.enabled_count(),
            syn_detectors: registry.count(), // TODO: separate counts
            clippy_detectors: 0,             // TODO: get from clippy analyzer
        }
    }

    /// Trigger a manual workspace scan
    #[allow(dead_code)]
    pub async fn scan_workspace(&self) -> Option<ScanResult> {
        let scanner = self.file_scanner.lock().await;
        let mut registry = self.detector_registry.lock().await;
        Some(scanner.scan_workspace(&mut registry).await)
    }

    /// Invalidate cache for a specific file
    #[allow(dead_code)]
    pub async fn invalidate_file_cache(&self, file_path: &std::path::PathBuf) {
        let registry = self.detector_registry.lock().await;
        registry.invalidate_cache(file_path).await;
    }

    /// Clear all analysis caches
    #[allow(dead_code)]
    pub async fn clear_all_caches(&self) {
        let registry = self.detector_registry.lock().await;
        registry.clear_cache().await;
    }
}

/// Statistics about the detector system
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct DetectorStats {
    pub total_detectors: usize,
    pub enabled_detectors: usize,
    pub syn_detectors: usize,
    pub clippy_detectors: usize,
}

/// Create a default detector registry with all available detectors
fn create_default_registry() -> DetectorRegistry {
    DetectorRegistryBuilder::new()
        // Syn-based detectors (immediate analysis)
        // .with_syn_detector(UnsafeMathDetector::default())
        .with_syn_detector(MissingSignerDetector::default())
        .with_syn_detector(ManualLamportsZeroingDetector::default())
        .with_syn_detector(SysvarAccountDetector::default())
        .with_syn_detector(ImmutableAccountMutatedDetector::default())
        .with_syn_detector(MissingInitspaceDetector::default())
        .with_syn_detector(InstructionAttributeUnusedDetector::default())
        .with_syn_detector(InstructionAttributeInvalidDetector::default())
        .with_syn_detector(MissingCheckCommentDetector::default())
        // Clippy-style detectors (comprehensive analysis)
        .with_clippy_detector(ClippyUncheckedArithmeticDetector::new())
        .build()
}
