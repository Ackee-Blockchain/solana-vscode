use super::detector::Detector;
use super::detector_config::DetectorConfig;
use crate::core::utilities::{DiagnosticBuilder, anchor_patterns::AnchorPatterns};
use log::{debug, info};
use std::path::PathBuf;
use syn::Ident;
use syn::spanned::Spanned;
use syn::{parse_str, visit::Visit};
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};

#[derive(Default)]
pub struct MissingSignerDetector {
    diagnostics: Vec<Diagnostic>,
    config: DetectorConfig,
    contexts: Vec<Ident>,
}

impl MissingSignerDetector {
    #[allow(dead_code)]
    pub fn with_config(config: DetectorConfig) -> Self {
        Self {
            diagnostics: Vec::new(),
            config,
            contexts: Vec::new(),
        }
    }
}

impl Detector for MissingSignerDetector {
    fn id(&self) -> &'static str {
        "MISSING_SIGNER"
    }

    fn name(&self) -> &'static str {
        "Missing Signer Check"
    }

    fn description(&self) -> &'static str {
        "Detects Anchor programs that have no signer accounts, which could allow unauthorized access"
    }

    fn message(&self) -> &'static str {
        "Program instruction has no signer. Consider adding a Signer<'info> field to ensure proper authorization."
    }

    fn default_severity(&self) -> DiagnosticSeverity {
        DiagnosticSeverity::WARNING
    }

    fn analyze(&mut self, content: &str, file_path: Option<&PathBuf>) -> Vec<Diagnostic> {
        self.diagnostics.clear();
        self.contexts.clear();

        info!("Starting Missing Signer analysis");

        // Basic parsing of the file
        if let Ok(syntax_tree) = parse_str::<syn::File>(content) {
            self.visit_file(&syntax_tree);
        }

        self.diagnostics.clone()
    }
}

impl<'ast> Visit<'ast> for MissingSignerDetector {
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        if AnchorPatterns::is_program_module(node) {
            for item in node.content.as_ref().unwrap().1.clone() {
                match item {
                    syn::Item::Fn(item_fn) => {
                        self.visit_item_fn(&item_fn);
                    }
                    _ => continue,
                }
            }
        }
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        if let syn::Visibility::Public(_) = node.vis {
            for param in &node.sig.inputs {
                if let Some(inner_segment) = AnchorPatterns::extract_context_type(param) {
                    debug!("Found inner segment Ident: {:?}", inner_segment.ident);
                    self.contexts.push(inner_segment.ident.clone());
                }
            }
        }
    }

    fn visit_file(&mut self, file: &'ast syn::File) {
        for item in &file.items {
            match item {
                syn::Item::Mod(item_mod) => {
                    self.visit_item_mod(item_mod);
                }
                _ => continue,
            }
        }
    }
}
