use tower_lsp::{
    lsp_types::{
        DidChangeTextDocumentParams, DidOpenTextDocumentParams, InitializeParams, InitializeResult, PositionEncodingKind, ServerCapabilities, ServerInfo, TextDocumentItem, TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions
    }, Client, LanguageServer
};
#[derive(Debug, Clone)]
pub struct Backend {
    client: Client,
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
        Backend{ client }
    }

    async fn on_change(&self, params: TextDocumentItem) {
        todo!()
    }
}
