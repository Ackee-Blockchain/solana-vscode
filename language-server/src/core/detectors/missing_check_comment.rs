use super::detector::Detector;
use super::detector_config::DetectorConfig;
use crate::core::utilities::{DiagnosticBuilder, anchor_patterns::AnchorPatterns};
use std::path::PathBuf;
use syn::spanned::Spanned;
use syn::{parse_str, visit::Visit, Fields, Type, TypePath};
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};

#[derive(Default)]
pub struct MissingCheckCommentDetector {
    diagnostics: Vec<Diagnostic>,
    config: DetectorConfig,
}

impl MissingCheckCommentDetector {
    #[allow(dead_code)]
    pub fn with_config(config: DetectorConfig) -> Self {
        Self {
            diagnostics: Vec::new(),
            config,
        }
    }

    /// Check if a field type is AccountInfo or UncheckedAccount
    fn is_unchecked_account_type(&self, field: &syn::Field) -> Option<String> {
        if let Type::Path(TypePath { path, .. }) = &field.ty {
            if let Some(segment) = path.segments.last() {
                let type_name = segment.ident.to_string();
                if type_name == "AccountInfo" || type_name == "UncheckedAccount" {
                    return Some(type_name);
                }
            }
        }
        None
    }

    /// Check if field has a /// CHECK: doc comment
    fn has_check_doc_comment(&self, field: &syn::Field) -> bool {
        for attr in &field.attrs {
            if attr.path().is_ident("doc") {
                if let syn::Meta::NameValue(meta_name_value) = &attr.meta {
                    if let syn::Expr::Lit(expr_lit) = &meta_name_value.value {
                        if let syn::Lit::Str(lit_str) = &expr_lit.lit {
                            let doc_content = lit_str.value();
                            let trimmed_content = doc_content.trim();
                            if trimmed_content.starts_with("CHECK:") {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    }

    /// Generate a helpful suggestion message with example
    fn get_suggestion_message(&self, field_name: &str, account_type: &str) -> String {
        format!(
            "Missing /// CHECK: doc comment for {} field '{}'. Add a doc comment explaining why this account doesn't need validation. Example:\n/// CHECK: This account is used for [explain purpose and why it's safe]",
            account_type, field_name
        )
    }
}

impl Detector for MissingCheckCommentDetector {
    fn id(&self) -> &'static str {
        "MISSING_CHECK_COMMENT"
    }

    fn name(&self) -> &'static str {
        "Missing CHECK Comment"
    }

    fn description(&self) -> &'static str {
        "Detects AccountInfo and UncheckedAccount fields without required /// CHECK: doc comments"
    }

    fn message(&self) -> &'static str {
        "Missing /// CHECK: doc comment for unchecked account"
    }

    fn default_severity(&self) -> DiagnosticSeverity {
        DiagnosticSeverity::ERROR
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

impl<'ast> Visit<'ast> for MissingCheckCommentDetector {
    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        // Only check structs with #[derive(Accounts)]
        if !AnchorPatterns::is_accounts_struct(node) {
            return;
        }

        // Check each field for unchecked account types
        if let Fields::Named(fields) = &node.fields {
            for field in &fields.named {
                if let Some(account_type) = self.is_unchecked_account_type(field) {
                    if !self.has_check_doc_comment(field) {
                        let field_name = field
                            .ident
                            .as_ref()
                            .map(|ident| ident.to_string())
                            .unwrap_or_else(|| "unknown".to_string());

                        let severity = self
                            .config
                            .severity_override
                            .unwrap_or(self.default_severity());

                        let message = self.get_suggestion_message(&field_name, &account_type);

                        self.diagnostics.push(DiagnosticBuilder::create(
                            DiagnosticBuilder::create_range_from_span(field.span()),
                            message,
                            severity,
                            self.id().to_string(),
                            None,
                        ));
                    }
                }
            }
        }

        // Continue visiting children
        syn::visit::visit_item_struct(self, node);
    }
} 