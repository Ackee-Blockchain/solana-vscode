use super::detector::Detector;
use crate::core::utilities::DiagnosticBuilder;
use syn::spanned::Spanned;
use syn::{Attribute, DeriveInput, Fields, Meta, parse_str, visit::Visit};
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};

pub struct MissingSignerDetector {
    diagnostics: Vec<Diagnostic>,
}

impl MissingSignerDetector {
    pub fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
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

    /// Check if a struct has the #[derive(Accounts)] attribute
    fn has_accounts_derive(&self, attrs: &[Attribute]) -> bool {
        for attr in attrs {
            if let Meta::List(meta_list) = &attr.meta {
                if meta_list.path.is_ident("derive") {
                    let tokens = meta_list.tokens.to_string();
                    if tokens.contains("Accounts") {
                        return true;
                    }
                }
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

    fn analyze(&mut self, content: &str) -> Vec<Diagnostic> {
        self.diagnostics.clear();

        if let Ok(syntax_tree) = parse_str::<syn::File>(content) {
            self.visit_file(&syntax_tree);
        }

        self.diagnostics.clone()
    }

    fn should_run(&self, content: &str) -> bool {
        // Run on Anchor files that contain #[derive(Accounts)]
        (content.contains("anchor_lang") || content.contains("anchor_spl"))
            && content.contains("#[derive(Accounts)]")
    }
}

impl<'ast> Visit<'ast> for MissingSignerDetector {
    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        // Check if this struct has #[derive(Accounts)]
        if !self.has_accounts_derive(&node.attrs) {
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
            self.diagnostics.push(DiagnosticBuilder::create(
                DiagnosticBuilder::create_range_from_span(node.span()),
                self.message().to_string(),
                self.default_severity(),
                self.id().to_string(),
                None,
            ));
        }

        // Continue visiting children
        syn::visit::visit_item_struct(self, node);
    }
}
