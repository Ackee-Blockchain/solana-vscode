use super::detector::Detector;
use super::detector_config::DetectorConfig;
use crate::core::utilities::{DiagnosticBuilder, anchor_patterns::AnchorPatterns};
use std::path::PathBuf;
use syn::spanned::Spanned;
use syn::{parse_str, visit::Visit};
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};

#[derive(Default)]
pub struct MissingInitspaceDetector {
    diagnostics: Vec<Diagnostic>,
    config: DetectorConfig,
}

impl MissingInitspaceDetector {
    #[allow(dead_code)]
    pub fn with_config(config: DetectorConfig) -> Self {
        Self {
            diagnostics: Vec::new(),
            config,
        }
    }
}

impl Detector for MissingInitspaceDetector {
    fn id(&self) -> &'static str {
        "MISSING_INITSPACE"
    }

    fn name(&self) -> &'static str {
        "Missing InitSpace macro"
    }

    fn description(&self) -> &'static str {
        "Detects Anchor accounts structs that don't use the #[derive(InitSpace)] macro"
    }

    fn message(&self) -> &'static str {
        "Accounts struct has no #[derive(InitSpace)] macro. Consider adding it for proper space allocation."
    }

    fn default_severity(&self) -> DiagnosticSeverity {
        DiagnosticSeverity::WARNING
    }

    fn analyze(&mut self, content: &str, _file_path: Option<&PathBuf>) -> Vec<Diagnostic> {
        self.diagnostics.clear();

        if let Ok(syntax_tree) = parse_str::<syn::File>(content) {
            self.visit_file(&syntax_tree);
        }

        self.diagnostics.clone()
    }
}

impl<'ast> Visit<'ast> for MissingInitspaceDetector {
    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        if !AnchorPatterns::is_account_struct(node) {
            return;
        }

        // Check if the struct has the #[derive(InitSpace)] macro
        let has_initspace_macro = node.attrs.iter().any(|attr| {
            if attr.path().is_ident("derive") {
                if let Ok(meta) = attr.meta.require_list() {
                    let tokens = meta.tokens.to_string();
                    return tokens.contains("InitSpace");
                }
            }
            false
        });

        if !has_initspace_macro {
            let severity = self
                .config
                .severity_override
                .unwrap_or(self.default_severity());

            // Create a range that covers the entire line
            let line = node.span().start().line as u32;
            let range = DiagnosticBuilder::create_range_from_line(line);

            self.diagnostics.push(DiagnosticBuilder::create(
                range,
                self.message().to_string(),
                severity,
                self.id().to_string(),
                None,
            ));
        }

        // Continue visiting children
        syn::visit::visit_item_struct(self, node);
    }
}
