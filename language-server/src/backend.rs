use crate::core::{
    DetectorInfo, DetectorRegistry, DetectorRegistryBuilder, FileScanner,
    ImmutableAccountMutatedDetector, InstructionAttributeInvalidDetector,
    InstructionAttributeUnusedDetector, ManualLamportsZeroingDetector, MissingCheckCommentDetector,
    MissingInitspaceDetector, MissingSignerDetector, ScanCompleteNotification, ScanResult,
    ScanSummary, SysvarAccountDetector, UnsafeMathDetector,
};
use crate::dylint_runner::DylintRunner;
use log::{info, warn};
use std::path::PathBuf;
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
    dylint_runner: Arc<Mutex<Option<DylintRunner>>>,
    workspace_root: Arc<Mutex<Option<PathBuf>>>,
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
                    // Store workspace root
                    {
                        let mut root = self.workspace_root.lock().await;
                        *root = Some(path.clone());
                    }

                    let mut scanner = self.file_scanner.lock().await;
                    scanner.set_workspace_root(path.clone());

                    // Try to initialize DylintRunner
                    // Get project root from the language server binary location
                    // Binary is at: <project>/extension/bin/language-server
                    // Project root is: <project>/
                    let project_root = std::env::current_exe()
                        .ok()
                        .and_then(|exe| exe.parent().map(|p| p.to_path_buf())) // extension/bin/
                        .and_then(|bin| bin.parent().map(|p| p.to_path_buf())) // extension/
                        .and_then(|ext| ext.parent().map(|p| p.to_path_buf())) // project root/
                        .unwrap_or_else(|| path.clone());

                    info!("🔍 Looking for dylint lints in: {:?}", project_root);
                    match DylintRunner::new(&project_root) {
                        Ok(runner) => {
                            info!("✅ DylintRunner initialized successfully");
                            let mut dylint = self.dylint_runner.lock().await;
                            *dylint = Some(runner);
                        }
                        Err(e) => {
                            warn!(
                                "⚠️ Failed to initialize DylintRunner: {}. Dylint lints will not be available.",
                                e
                            );
                            warn!(
                                "   To enable dylint lints, build them by running: cd lints && ./build_all_lints.sh"
                            );
                        }
                    }

                    // Perform initial workspace scan
                    info!("Performing initial workspace scan...");
                    let mut registry = self.detector_registry.lock().await;
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
                scanner.set_workspace_root(path);

                // Perform initial workspace scan
                info!("Performing initial workspace scan...");
                let mut registry = self.detector_registry.lock().await;
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

        // First, analyze the changed file to provide immediate feedback
        self.on_change(text_document).await;

        // Then trigger a full workspace scan to update all diagnostics
        info!("File change detected, performing full workspace scan...");
        let scan_result = {
            let scanner = self.file_scanner.lock().await;
            let mut registry = self.detector_registry.lock().await;
            scanner.scan_workspace(&mut registry).await
        };

        // Publish diagnostics for all files with issues
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

    async fn execute_command(
        &self,
        params: tower_lsp::lsp_types::ExecuteCommandParams,
    ) -> JsonRpcResult<Option<serde_json::Value>> {
        match params.command.as_str() {
            "solana.scanWorkspace" => {
                info!("Manual workspace scan triggered");
                let scan_result = {
                    let scanner = self.file_scanner.lock().await;
                    let mut registry = self.detector_registry.lock().await;
                    scanner.scan_workspace(&mut registry).await
                };

                // Publish diagnostics for all files with issues
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
            "solana.reloadDetectors" => {
                info!("Reloading all detectors");

                // Create a new detector registry with fresh detector instances
                let new_registry = create_default_registry();

                // Replace the existing registry
                {
                    let mut registry = self.detector_registry.lock().await;
                    *registry = new_registry;
                }

                // Trigger a full workspace scan with the new detectors
                let scan_result = {
                    let scanner = self.file_scanner.lock().await;
                    let mut registry = self.detector_registry.lock().await;
                    scanner.scan_workspace(&mut registry).await
                };

                // Publish diagnostics for all files with issues
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
                    "message": "Detectors reloaded and workspace rescanned"
                })))
            }
            "solana.runDylintLints" => {
                info!("Running dylint lints on workspace");

                // Check if DylintRunner is initialized
                let dylint_runner = self.dylint_runner.lock().await;
                let Some(runner) = dylint_runner.as_ref() else {
                    warn!("DylintRunner not initialized. Cannot run lints.");
                    return Ok(Some(serde_json::json!({
                        "success": false,
                        "error": "DylintRunner not initialized. Make sure lints are built."
                    })));
                };

                // Get workspace root
                let workspace_root = self.workspace_root.lock().await;
                let Some(workspace_path) = workspace_root.as_ref() else {
                    warn!("No workspace root set");
                    return Ok(Some(serde_json::json!({
                        "success": false,
                        "error": "No workspace root set"
                    })));
                };

                // Run dylint
                match runner.run_lints(workspace_path).await {
                    Ok(diagnostics) => {
                        info!("✅ Dylint found {} diagnostics", diagnostics.len());

                        // Group diagnostics by file
                        let mut diagnostics_by_file: std::collections::HashMap<
                            PathBuf,
                            Vec<tower_lsp::lsp_types::Diagnostic>,
                        > = std::collections::HashMap::new();

                        for diag in diagnostics {
                            // Convert relative path to absolute by joining with workspace path
                            let file_path = if PathBuf::from(&diag.file_name).is_absolute() {
                                PathBuf::from(&diag.file_name)
                            } else {
                                workspace_path.join(&diag.file_name)
                            };

                            diagnostics_by_file
                                .entry(file_path)
                                .or_insert_with(Vec::new)
                                .push(diag.to_lsp_diagnostic());
                        }

                        let total_diagnostics: usize =
                            diagnostics_by_file.values().map(|v| v.len()).sum();
                        let total_files = diagnostics_by_file.len();

                        // Publish diagnostics for each file
                        for (file_path, file_diagnostics) in diagnostics_by_file {
                            if let Ok(uri) = tower_lsp::lsp_types::Url::from_file_path(&file_path) {
                                info!(
                                    "📍 Publishing {} diagnostics for: {}",
                                    file_diagnostics.len(),
                                    uri
                                );
                                for diag in &file_diagnostics {
                                    info!(
                                        "   - Line {}: {}",
                                        diag.range.start.line + 1,
                                        diag.message
                                    );
                                }
                                self.client
                                    .publish_diagnostics(uri, file_diagnostics, None)
                                    .await;
                            } else {
                                warn!("❌ Failed to convert path to URI: {}", file_path.display());
                            }
                        }

                        info!(
                            "📤 Published {} diagnostics across {} files",
                            total_diagnostics, total_files
                        );

                        Ok(Some(serde_json::json!({
                            "success": true,
                            "total_diagnostics": total_diagnostics,
                            "total_files": total_files
                        })))
                    }
                    Err(e) => {
                        warn!("Failed to run dylint: {}", e);
                        Ok(Some(serde_json::json!({
                            "success": false,
                            "error": format!("{}", e)
                        })))
                    }
                }
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
            dylint_runner: Arc::new(Mutex::new(None)),
            workspace_root: Arc::new(Mutex::new(None)),
        }
    }

    async fn on_change(&self, params: TextDocumentItem) {
        // Run security analysis
        let diagnostics = {
            let mut registry = self.detector_registry.lock().await;
            let file_path = params.uri.to_file_path().ok();
            registry.analyze(&params.text, file_path.as_ref())
        };

        // Publish diagnostics to the client
        self.client
            .publish_diagnostics(params.uri.clone(), diagnostics, Some(params.version))
            .await;
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
        }
    }

    /// Trigger a manual workspace scan
    #[allow(dead_code)]
    pub async fn scan_workspace(&self) -> Option<ScanResult> {
        let scanner = self.file_scanner.lock().await;
        let mut registry = self.detector_registry.lock().await;
        Some(scanner.scan_workspace(&mut registry).await)
    }
}

/// Statistics about the detector system
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct DetectorStats {
    pub total_detectors: usize,
    pub enabled_detectors: usize,
}

/// Create a default detector registry with all available detectors
fn create_default_registry() -> DetectorRegistry {
    info!("Creating new detector registry with all detectors");
    let registry = DetectorRegistryBuilder::new()
        // .with_detector(UnsafeMathDetector::default())
        .with_detector(MissingSignerDetector::default()) // Ensure MissingSignerDetector is included
        .with_detector(ManualLamportsZeroingDetector::default())
        .with_detector(SysvarAccountDetector::default())
        .with_detector(ImmutableAccountMutatedDetector::default())
        .with_detector(MissingInitspaceDetector::default())
        .with_detector(InstructionAttributeUnusedDetector::default())
        .with_detector(InstructionAttributeInvalidDetector::default())
        .with_detector(MissingCheckCommentDetector::default())
        .build();

    info!(
        "Detector registry created with {} detectors",
        registry.count()
    );
    registry
}
