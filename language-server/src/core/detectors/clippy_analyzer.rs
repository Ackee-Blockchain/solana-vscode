use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;
use tower_lsp::lsp_types::Diagnostic;

/// Information extracted from cargo check output
#[derive(Debug, Clone)]
pub struct TypeInfo {
    pub file_path: PathBuf,
    pub line: u32,
    pub column: u32,
    pub expr_type: String,
    pub span_start: u32,
    pub span_end: u32,
}

pub struct ClippyAnalyzer {
    cache: Arc<TokioMutex<HashMap<PathBuf, Vec<Diagnostic>>>>,
    workspace_root: Option<PathBuf>,
    type_info_cache: Arc<TokioMutex<HashMap<PathBuf, Vec<TypeInfo>>>>,
}

impl ClippyAnalyzer {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(TokioMutex::new(HashMap::new())),
            workspace_root: None,
            type_info_cache: Arc::new(TokioMutex::new(HashMap::new())),
        }
    }

    // TODO
}
