use crate::core::detector::Detector;
use crate::core::{DetectorInfo, DetectorRegistry};
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_lsp::{
    Client, LanguageServer,
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
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(
        &self,
        _: InitializeParams,
    ) -> Result<InitializeResult, tower_lsp::jsonrpc::Error> {
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
}

impl Backend {
    pub fn new(client: Client) -> Backend {
        Backend {
            client,
            detector_registry: Arc::new(Mutex::new(create_default_registry())),
        }
    }

    async fn on_change(&self, _params: TextDocumentItem) {
        todo!()
    }

    /// Get information about all registered detectors
    pub async fn list_detectors(&self) -> Vec<DetectorInfo> {
        let registry = self.detector_registry.lock().await;
        registry.list_detectors()
    }

    /// Enable or disable a specific detector
    pub async fn set_detector_enabled(&self, detector_id: &str, enabled: bool) {
        let mut registry = self.detector_registry.lock().await;
        if enabled {
            registry.enable(detector_id);
        } else {
            registry.disable(detector_id);
        }
    }

    /// Get detector statistics
    pub async fn get_detector_stats(&self) -> DetectorStats {
        let registry = self.detector_registry.lock().await;
        DetectorStats {
            total_detectors: registry.count(),
            enabled_detectors: registry.enabled_count(),
        }
    }
}

/// Statistics about the detector system
#[derive(Debug, Clone)]
pub struct DetectorStats {
    pub total_detectors: usize,
    pub enabled_detectors: usize,
}

/// Create a default detector registry with all available detectors
fn create_default_registry() -> DetectorRegistry {
    use crate::core::{DetectorRegistryBuilder, UnsafeMathDetector};

    DetectorRegistryBuilder::new()
        .with_detector(UnsafeMathDetector::new())
        .build()
}
