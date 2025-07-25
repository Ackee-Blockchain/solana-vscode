use super::detector::Detector;
use super::detector_config::DetectorConfig;
use crate::core::utilities::{DiagnosticBuilder, anchor_patterns::AnchorPatterns};
use std::path::PathBuf;
use syn::spanned::Spanned;
use syn::{Fields, parse_str, visit::Visit};
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};

#[derive(Default)]
pub struct MissingSignerDetector {
    diagnostics: Vec<Diagnostic>,
    config: DetectorConfig,
}

impl MissingSignerDetector {
    #[allow(dead_code)]
    pub fn with_config(config: DetectorConfig) -> Self {
        Self {
            diagnostics: Vec::new(),
            config,
        }
    }

    /// Check if a field has the Signer type
    fn is_signer_field(&self, field: &syn::Field) -> bool {
        if let syn::Type::Path(type_path) = &field.ty {
            if let Some(segment) = type_path.path.segments.last() {
                return segment.ident == "Signer";
            }
        }
        false
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
        "Detects Anchor accounts structs that have no signer accounts, which could allow unauthorized access"
    }

    fn message(&self) -> &'static str {
        "Accounts struct has no signer. Consider adding a Signer<'info> field to ensure proper authorization."
    }

    fn default_severity(&self) -> DiagnosticSeverity {
        DiagnosticSeverity::WARNING
    }

    fn analyze(&mut self, content: &str, _file_path: Option<&PathBuf>) -> Vec<Diagnostic> {
        self.diagnostics.clear();

        // Run default detection logic
        if let Ok(syntax_tree) = parse_str::<syn::File>(content) {
            self.visit_file(&syntax_tree);
        }

        self.diagnostics.clone()
    }
}

impl<'ast> Visit<'ast> for MissingSignerDetector {
    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        // Check if this struct has #[derive(Accounts)]
        if !AnchorPatterns::is_accounts_struct(node) {
            return;
        }

        // Check if any field is a Signer
        let mut has_signer = false;

        if let Fields::Named(fields) = &node.fields {
            for field in &fields.named {
                if self.is_signer_field(field) {
                    has_signer = true;
                    break;
                }
            }
        }

        // If no signer found, create a diagnostic
        if !has_signer {
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
