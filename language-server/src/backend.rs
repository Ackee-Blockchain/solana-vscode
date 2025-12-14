use crate::core::dylint::constants::REQUIRED_NIGHTLY_VERSION;
use crate::core::{
    DetectorInfo, DetectorRegistry, DetectorRegistryBuilder, DetectorStatus,
    DetectorStatusNotification, DylintDetectorManager, FileScanner,
    ImmutableAccountMutatedDetector, InstructionAttributeInvalidDetector,
    InstructionAttributeUnusedDetector, ManualLamportsZeroingDetector, MissingCheckCommentDetector,
    MissingInitspaceDetector, MissingSignerDetector, ScanCompleteNotification, ScanResult,
    ScanSummary, SysvarAccountDetector,
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
        DidChangeTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
        InitializeParams, InitializeResult, PositionEncodingKind, SaveOptions, ServerCapabilities,
        ServerInfo, TextDocumentItem, TextDocumentSyncCapability, TextDocumentSyncKind,
        TextDocumentSyncOptions, TextDocumentSyncSaveOptions,
    },
};

#[derive(Debug, Clone)]
pub struct Backend {
    client: Client,
    detector_registry: Arc<Mutex<DetectorRegistry>>,
    file_scanner: Arc<Mutex<FileScanner>>,
    dylint_runner: Option<Arc<DylintRunner>>,
    dylint_manager: Arc<Mutex<Option<DylintDetectorManager>>>,
    workspace_root: Arc<Mutex<Option<PathBuf>>>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(
        &self,
        params: InitializeParams,
    ) -> Result<InitializeResult, tower_lsp::jsonrpc::Error> {
        // Set up workspace root if provided
        if let Some(workspace_folders) = params.workspace_folders
            && let Some(folder) = workspace_folders.first()
            && let Ok(path) = folder.uri.to_file_path()
        {
            // Store workspace root for dylint
            *self.workspace_root.lock().await = Some(path.clone());

            let mut scanner = self.file_scanner.lock().await;
            scanner.set_workspace_root(path.clone());

            // Perform initial workspace scan
            info!("Performing initial workspace scan...");
            let mut registry = self.detector_registry.lock().await;
            let scan_result = scanner.scan_workspace(&mut registry).await;
            drop(registry); // Release registry lock before initializing dylint
            drop(scanner); // Release scanner lock

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

            // Publish diagnostics for ALL scanned files (including empty diagnostics for fixed files)
            for file_info in &scan_result.rust_files {
                if let Ok(uri) = tower_lsp::lsp_types::Url::from_file_path(&file_info.path) {
                    self.client
                        .publish_diagnostics(uri, file_info.diagnostics.clone(), None)
                        .await;
                }
            }

            // Send scan results to extension (initial scan, not manual)
            let scan_summary = ScanSummary::from_scan_result(&scan_result, false);
            self.client
                .send_notification::<ScanCompleteNotification>(scan_summary)
                .await;

            // Initialize dylint detectors on project open
            info!("[Extension Dylint] Initializing detectors on project open...");

            // Notify extension that detectors are initializing
            self.client
                .send_notification::<DetectorStatusNotification>(DetectorStatus {
                    status: "initializing".to_string(),
                    message: "Initializing security detectors...".to_string(),
                })
                .await;

            Self::ensure_dylint_detectors_initialized(self).await;

            // Run dylint in background and merge with syn diagnostics
            if let Some(dylint_runner) = &self.dylint_runner {
                let runner = Arc::clone(dylint_runner);
                let workspace = path.clone();
                let client = self.client.clone();
                let file_list: Vec<(std::path::PathBuf, Vec<tower_lsp::lsp_types::Diagnostic>)> =
                    scan_result
                        .rust_files
                        .iter()
                        .map(|f| (f.path.clone(), f.diagnostics.clone()))
                        .collect();

                tokio::spawn(async move {
                    info!("Running dylint on project open...");

                    // Notify that detectors are running
                    client
                        .send_notification::<DetectorStatusNotification>(DetectorStatus {
                            status: "running".to_string(),
                            message: "Running security detectors...".to_string(),
                        })
                        .await;

                    match runner.run_lints(&workspace).await {
                        Ok(dylint_diagnostics) => {
                            info!(
                                "Dylint found {} total issues on project open",
                                dylint_diagnostics.len()
                            );

                            // Merge dylint diagnostics with syn diagnostics for each file
                            for (file_path, syn_diagnostics) in file_list {
                                if let Ok(uri) =
                                    tower_lsp::lsp_types::Url::from_file_path(&file_path)
                                {
                                    // Filter dylint diagnostics for this file
                                    let dylint_file_diagnostics: Vec<_> = dylint_diagnostics
                                        .iter()
                                        .filter(|d| {
                                            let path_str = file_path.to_string_lossy();
                                            path_str.ends_with(&d.file_name)
                                                || path_str.contains(&d.file_name)
                                        })
                                        .map(|d| d.to_lsp_diagnostic())
                                        .collect();

                                    if !dylint_file_diagnostics.is_empty() {
                                        // Merge syn and dylint diagnostics
                                        let mut merged_diagnostics = syn_diagnostics.clone();
                                        merged_diagnostics.extend(dylint_file_diagnostics.clone());

                                        info!(
                                            "Publishing {} total diagnostics ({} syn + {} dylint) for {}",
                                            merged_diagnostics.len(),
                                            syn_diagnostics.len(),
                                            dylint_file_diagnostics.len(),
                                            file_path.display()
                                        );

                                        // Publish merged diagnostics
                                        client
                                            .publish_diagnostics(uri, merged_diagnostics, None)
                                            .await;
                                    }
                                }
                            }

                            // Notify complete
                            client
                                .send_notification::<DetectorStatusNotification>(DetectorStatus {
                                    status: "complete".to_string(),
                                    message: "Security scan complete".to_string(),
                                })
                                .await;
                        }
                        Err(e) => {
                            info!("Dylint failed on project open: {}", e);

                            // Notify complete even on error
                            client
                                .send_notification::<DetectorStatusNotification>(DetectorStatus {
                                    status: "complete".to_string(),
                                    message: "Security scan complete".to_string(),
                                })
                                .await;
                        }
                    }
                });
            }
        } else if let Some(root_uri) = params.root_uri
            && let Ok(path) = root_uri.to_file_path()
        {
            // Store workspace root for dylint
            *self.workspace_root.lock().await = Some(path.clone());

            let mut scanner = self.file_scanner.lock().await;
            scanner.set_workspace_root(path.clone());

            // Perform initial workspace scan
            info!("Performing initial workspace scan...");
            let mut registry = self.detector_registry.lock().await;
            let scan_result = scanner.scan_workspace(&mut registry).await;
            drop(registry); // Release registry lock before initializing dylint
            drop(scanner); // Release scanner lock

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

            // Publish diagnostics for ALL scanned files (including empty diagnostics for fixed files)
            for file_info in &scan_result.rust_files {
                if let Ok(uri) = tower_lsp::lsp_types::Url::from_file_path(&file_info.path) {
                    self.client
                        .publish_diagnostics(uri, file_info.diagnostics.clone(), None)
                        .await;
                }
            }

            // Send scan results to extension (initial scan, not manual)
            let scan_summary = ScanSummary::from_scan_result(&scan_result, false);
            self.client
                .send_notification::<ScanCompleteNotification>(scan_summary)
                .await;

            // Initialize dylint detectors on project open
            info!("[Extension Dylint] Initializing detectors on project open...");

            // Notify extension that detectors are initializing
            self.client
                .send_notification::<DetectorStatusNotification>(DetectorStatus {
                    status: "initializing".to_string(),
                    message: "Initializing security detectors...".to_string(),
                })
                .await;

            Self::ensure_dylint_detectors_initialized(self).await;

            // Run dylint in background and merge with syn diagnostics
            if let Some(dylint_runner) = &self.dylint_runner {
                let runner = Arc::clone(dylint_runner);
                let workspace = path.clone();
                let client = self.client.clone();
                let file_list: Vec<(std::path::PathBuf, Vec<tower_lsp::lsp_types::Diagnostic>)> =
                    scan_result
                        .rust_files
                        .iter()
                        .map(|f| (f.path.clone(), f.diagnostics.clone()))
                        .collect();

                tokio::spawn(async move {
                    info!("Running dylint on project open...");

                    // Notify that detectors are running
                    client
                        .send_notification::<DetectorStatusNotification>(DetectorStatus {
                            status: "running".to_string(),
                            message: "Running security detectors...".to_string(),
                        })
                        .await;

                    match runner.run_lints(&workspace).await {
                        Ok(dylint_diagnostics) => {
                            info!(
                                "Dylint found {} total issues on project open",
                                dylint_diagnostics.len()
                            );

                            // Merge dylint diagnostics with syn diagnostics for each file
                            for (file_path, syn_diagnostics) in file_list {
                                if let Ok(uri) =
                                    tower_lsp::lsp_types::Url::from_file_path(&file_path)
                                {
                                    // Filter dylint diagnostics for this file
                                    let dylint_file_diagnostics: Vec<_> = dylint_diagnostics
                                        .iter()
                                        .filter(|d| {
                                            let path_str = file_path.to_string_lossy();
                                            path_str.ends_with(&d.file_name)
                                                || path_str.contains(&d.file_name)
                                        })
                                        .map(|d| d.to_lsp_diagnostic())
                                        .collect();

                                    if !dylint_file_diagnostics.is_empty() {
                                        // Merge syn and dylint diagnostics
                                        let mut merged_diagnostics = syn_diagnostics.clone();
                                        merged_diagnostics.extend(dylint_file_diagnostics.clone());

                                        info!(
                                            "Publishing {} total diagnostics ({} syn + {} dylint) for {}",
                                            merged_diagnostics.len(),
                                            syn_diagnostics.len(),
                                            dylint_file_diagnostics.len(),
                                            file_path.display()
                                        );

                                        // Publish merged diagnostics
                                        client
                                            .publish_diagnostics(uri, merged_diagnostics, None)
                                            .await;
                                    }
                                }
                            }

                            // Notify complete
                            client
                                .send_notification::<DetectorStatusNotification>(DetectorStatus {
                                    status: "complete".to_string(),
                                    message: "Security scan complete".to_string(),
                                })
                                .await;
                        }
                        Err(e) => {
                            info!("Dylint failed on project open: {}", e);

                            // Notify complete even on error
                            client
                                .send_notification::<DetectorStatusNotification>(DetectorStatus {
                                    status: "complete".to_string(),
                                    message: "Security scan complete".to_string(),
                                })
                                .await;
                        }
                    }
                });
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
                        save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                            include_text: Some(true),
                        })),
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
        // Run detectors on file open
        self.on_change(params.text_document).await;
    }

    async fn did_change(&self, _params: DidChangeTextDocumentParams) {
        // Don't run detectors on change - only on save (consistent with rust-analyzer)
    }

    async fn did_save(&self, _params: DidSaveTextDocumentParams) {
        info!("File saved, reloading detectors and performing full workspace scan...");
        info!("[DEBUG] About to initialize dylint detectors...");

        // Create a new detector registry with fresh detector instances
        let new_registry = create_default_registry();

        // Replace the existing registry
        {
            let mut registry = self.detector_registry.lock().await;
            *registry = new_registry;
        }

        // Trigger a full workspace scan with the reloaded detectors
        let scan_result = {
            let scanner = self.file_scanner.lock().await;
            let mut registry = self.detector_registry.lock().await;
            scanner.scan_workspace(&mut registry).await
        };

        // Publish syn diagnostics for ALL scanned files
        for file_info in &scan_result.rust_files {
            if let Ok(uri) = tower_lsp::lsp_types::Url::from_file_path(&file_info.path) {
                self.client
                    .publish_diagnostics(uri, file_info.diagnostics.clone(), None)
                    .await;
            }
        }

        // Send scan results to extension (automatic scan on file save)
        let scan_summary = ScanSummary::from_scan_result(&scan_result, false);
        self.client
            .send_notification::<ScanCompleteNotification>(scan_summary)
            .await;

        // Initialize dylint detectors on first save (lazy initialization)
        // This checks if nightly is available and compiles/caches detectors

        // Notify extension that detectors are running
        self.client
            .send_notification::<DetectorStatusNotification>(DetectorStatus {
                status: "running".to_string(),
                message: "Running security detectors...".to_string(),
            })
            .await;

        Self::ensure_dylint_detectors_initialized(self).await;

        // Run dylint in background and merge with syn diagnostics
        if let Some(dylint_runner) = &self.dylint_runner {
            if let Some(workspace_root) = self.workspace_root.lock().await.as_ref() {
                let runner = Arc::clone(dylint_runner);
                let workspace = workspace_root.clone();
                let client = self.client.clone();
                // Create a simplified file list for dylint merging
                let file_list: Vec<(std::path::PathBuf, Vec<tower_lsp::lsp_types::Diagnostic>)> =
                    scan_result
                        .rust_files
                        .iter()
                        .map(|f| (f.path.clone(), f.diagnostics.clone()))
                        .collect();

                tokio::spawn(async move {
                    info!("Running dylint after save...");
                    match runner.run_lints(&workspace).await {
                        Ok(dylint_diagnostics) => {
                            info!(
                                "Dylint found {} total issues after save",
                                dylint_diagnostics.len()
                            );

                            // Merge dylint diagnostics with syn diagnostics for each file
                            for (file_path, syn_diagnostics) in file_list {
                                if let Ok(uri) =
                                    tower_lsp::lsp_types::Url::from_file_path(&file_path)
                                {
                                    // Filter dylint diagnostics for this file
                                    let dylint_file_diagnostics: Vec<_> = dylint_diagnostics
                                        .iter()
                                        .filter(|d| {
                                            let path_str = file_path.to_string_lossy();
                                            path_str.ends_with(&d.file_name)
                                                || path_str.contains(&d.file_name)
                                        })
                                        .map(|d| d.to_lsp_diagnostic())
                                        .collect();

                                    if !dylint_file_diagnostics.is_empty() {
                                        // Merge syn and dylint diagnostics
                                        let mut merged_diagnostics = syn_diagnostics.clone();
                                        merged_diagnostics.extend(dylint_file_diagnostics.clone());

                                        info!(
                                            "Publishing {} total diagnostics ({} syn + {} dylint) for {}",
                                            merged_diagnostics.len(),
                                            syn_diagnostics.len(),
                                            dylint_file_diagnostics.len(),
                                            file_path.display()
                                        );

                                        // Publish merged diagnostics
                                        client
                                            .publish_diagnostics(uri, merged_diagnostics, None)
                                            .await;
                                    }
                                }
                            }

                            // Notify complete
                            client
                                .send_notification::<DetectorStatusNotification>(DetectorStatus {
                                    status: "complete".to_string(),
                                    message: "Security scan complete".to_string(),
                                })
                                .await;
                        }
                        Err(e) => {
                            info!("Dylint failed after save: {}", e);

                            // Notify complete even on error
                            client
                                .send_notification::<DetectorStatusNotification>(DetectorStatus {
                                    status: "complete".to_string(),
                                    message: "Security scan complete".to_string(),
                                })
                                .await;
                        }
                    }
                });
            }
        }
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

                // Publish diagnostics for ALL scanned files (including empty diagnostics for fixed files)
                for file_info in &scan_result.rust_files {
                    if let Ok(uri) = tower_lsp::lsp_types::Url::from_file_path(&file_info.path) {
                        self.client
                            .publish_diagnostics(uri, file_info.diagnostics.clone(), None)
                            .await;
                    }
                }

                // Send scan results to extension (manual scan, show notification)
                let scan_summary = ScanSummary::from_scan_result(&scan_result, true);
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

                // Publish diagnostics for ALL scanned files (including empty diagnostics for fixed files)
                for file_info in &scan_result.rust_files {
                    if let Ok(uri) = tower_lsp::lsp_types::Url::from_file_path(&file_info.path) {
                        self.client
                            .publish_diagnostics(uri, file_info.diagnostics.clone(), None)
                            .await;
                    }
                }

                // Send scan results to extension (manual reload, show notification)
                let scan_summary = ScanSummary::from_scan_result(&scan_result, true);
                self.client
                    .send_notification::<ScanCompleteNotification>(scan_summary)
                    .await;

                Ok(Some(serde_json::json!({
                    "success": true,
                    "message": "Detectors reloaded and workspace rescanned"
                })))
            }
            _ => Ok(None),
        }
    }
}

impl Backend {
    pub fn new(client: Client) -> Backend {
        // Try to initialize dylint runner (for pre-compiled detectors)
        let dylint_runner = Self::try_init_dylint_runner();

        Backend {
            client,
            detector_registry: Arc::new(Mutex::new(create_default_registry())),
            file_scanner: Arc::new(Mutex::new(FileScanner::default())),
            dylint_runner,
            dylint_manager: Arc::new(Mutex::new(None)),
            workspace_root: Arc::new(Mutex::new(None)),
        }
    }

    /// Ensure dylint detectors are initialized (lazy initialization on first save)
    /// This checks if detectors have been initialized, and if not:
    /// 1. Checks if nightly Rust is available
    /// 2. Scans for detector source code in extension/detectors/
    /// 3. Checks cache for compiled versions matching current nightly
    /// 4. Compiles and caches if not present
    /// 5. Adds compiled detectors to dylint runner
    async fn ensure_dylint_detectors_initialized(&self) {
        info!("[DEBUG] ensure_dylint_detectors_initialized called");

        // Check if already initialized
        {
            let manager_lock = self.dylint_manager.lock().await;
            info!("[DEBUG] Got manager lock, checking initialization status...");
            if let Some(manager) = manager_lock.as_ref() {
                info!("[DEBUG] Manager exists, checking if initialized");
                if manager.is_initialized() {
                    // Already initialized, nothing to do
                    info!("[DEBUG] Manager already initialized, skipping");
                    return;
                }
                info!("[DEBUG] Manager not initialized yet");
            } else {
                info!("[DEBUG] Manager is None, will create new one");
            }
        }

        info!("[Extension Dylint] Initializing detectors on first save...");

        // Check if the required nightly version is available
        if !DylintDetectorManager::check_nightly_available() {
            warn!(
                "[Extension Dylint] Required nightly Rust version not available: {}",
                REQUIRED_NIGHTLY_VERSION
            );
            warn!(
                "[Extension Dylint] Install with: rustup toolchain install {}",
                REQUIRED_NIGHTLY_VERSION
            );
            return;
        }

        // Check if dylint-driver is available
        if !DylintDetectorManager::check_dylint_driver_available() {
            warn!("[Extension Dylint] dylint-driver not found");
            warn!("[Extension Dylint] Install with: cargo install cargo-dylint dylint-link");
            warn!(
                "[Extension Dylint] Then initialize: cargo +{} dylint --list",
                REQUIRED_NIGHTLY_VERSION
            );
            return;
        }

        // Get extension path (where detectors are bundled)
        let extension_path = match std::env::current_exe() {
            Ok(exe_path) => exe_path
                .parent()
                .and_then(|p| p.parent()) // bin/ -> extension/
                .map(|p| p.to_path_buf()),
            Err(_) => None,
        };

        let Some(extension_path) = extension_path else {
            warn!(
                "[Extension Dylint] Could not determine extension path, skipping detector initialization"
            );
            return;
        };

        info!(
            "[Extension Dylint] Initializing detectors from: {:?}",
            extension_path
        );

        // Create or get the manager
        let mut manager = match self.dylint_manager.lock().await.take() {
            Some(m) => m,
            None => match DylintDetectorManager::new() {
                Ok(m) => m,
                Err(e) => {
                    warn!("[Extension Dylint] Failed to create manager: {}", e);
                    return;
                }
            },
        };

        manager.set_extension_path(extension_path);

        // Initialize (will check cache and compile if needed)
        match manager.initialize().await {
            Ok(compiled_paths) => {
                if !compiled_paths.is_empty() {
                    info!(
                        "[Extension Dylint] Successfully initialized {} detector(s)",
                        compiled_paths.len()
                    );

                    // Add compiled detectors to dylint_runner
                    if let Some(dylint_runner) = &self.dylint_runner {
                        dylint_runner.add_workspace_detectors(compiled_paths);
                        info!("[Extension Dylint] Detectors added to dylint runner");
                    } else {
                        warn!(
                            "[Extension Dylint] Dylint runner not available, cannot add detectors"
                        );
                    }
                } else {
                    info!("[Extension Dylint] No detectors found in extension");
                }
            }
            Err(e) => {
                warn!("[Extension Dylint] Failed to initialize detectors: {}", e);
            }
        }

        // Store manager back
        *self.dylint_manager.lock().await = Some(manager);
    }

    fn try_init_dylint_runner() -> Option<Arc<DylintRunner>> {
        // Get the extension path (parent of language-server binary)
        let exe_path = std::env::current_exe().ok()?;
        let extension_path = exe_path.parent()?.parent()?; // bin/ -> extension/

        match DylintRunner::new(extension_path) {
            Ok(runner) => {
                // Runner can start empty and have detectors added later
                info!("Dylint runner initialized successfully");
                if runner.is_available() {
                    info!("Pre-compiled lints loaded: {:?}", runner.loaded_lints());
                } else {
                    info!("No pre-compiled lints found, but runner ready for extension detectors");
                }
                Some(Arc::new(runner))
            }
            Err(e) => {
                warn!(
                    "Failed to initialize dylint runner: {}. Dylint integration disabled.",
                    e
                );
                None
            }
        }
    }

    async fn on_change(&self, params: TextDocumentItem) {
        // 1. Run syn-based detectors (fast, real-time)
        let syn_diagnostics = {
            let mut registry = self.detector_registry.lock().await;
            let file_path = params.uri.to_file_path().ok();
            registry.analyze(&params.text, file_path.as_ref())
        };

        // 2. Publish syn-based diagnostics immediately
        self.client
            .publish_diagnostics(
                params.uri.clone(),
                syn_diagnostics.clone(),
                Some(params.version),
            )
            .await;

        // 3. Run dylint in background and merge diagnostics
        if let Some(dylint_runner) = &self.dylint_runner
            && let Some(workspace_root) = self.workspace_root.lock().await.as_ref()
        {
            let runner: Arc<DylintRunner> = Arc::clone(dylint_runner);
            let workspace = workspace_root.clone();
            let uri = params.uri.clone();
            let client = self.client.clone();
            let version = params.version;

            tokio::spawn(async move {
                info!("Running dylint lints on workspace: {}", workspace.display());
                match runner.run_lints(&workspace).await {
                    Ok(dylint_diagnostics) => {
                        info!(
                            "Dylint returned {} total diagnostics",
                            dylint_diagnostics.len()
                        );

                        // Filter diagnostics for this file
                        let file_path = uri.to_file_path().ok();
                        let dylint_file_diagnostics: Vec<_> = dylint_diagnostics
                            .iter()
                            .filter(|d| {
                                let matches = file_path
                                    .as_ref()
                                    .map(|p| {
                                        let path_str = p.to_string_lossy();
                                        let diagnostic_file = &d.file_name;
                                        let result = path_str.ends_with(diagnostic_file)
                                            || path_str.contains(diagnostic_file);
                                        info!(
                                            "Comparing {} with {} = {}",
                                            path_str, diagnostic_file, result
                                        );
                                        result
                                    })
                                    .unwrap_or(false);
                                matches
                            })
                            .map(|d| d.to_lsp_diagnostic())
                            .collect();

                        info!(
                            "Filtered to {} diagnostics for this file",
                            dylint_file_diagnostics.len()
                        );

                        if !dylint_file_diagnostics.is_empty() {
                            info!(
                                "Publishing {} dylint issues for file",
                                dylint_file_diagnostics.len()
                            );

                            // Merge syn and dylint diagnostics
                            let mut merged_diagnostics = syn_diagnostics;
                            merged_diagnostics.extend(dylint_file_diagnostics);

                            info!(
                                "Publishing {} total diagnostics (syn + dylint)",
                                merged_diagnostics.len()
                            );

                            // Publish merged diagnostics
                            client
                                .publish_diagnostics(uri, merged_diagnostics, Some(version))
                                .await;
                        }
                    }
                    Err(e) => {
                        info!("Dylint failed: {}", e);
                    }
                }
            });
        }
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
