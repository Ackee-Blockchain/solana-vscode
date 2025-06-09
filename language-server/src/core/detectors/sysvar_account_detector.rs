use super::detector::Detector;
use super::detector_config::DetectorConfig;
use crate::core::utilities::DiagnosticBuilder;
use syn::spanned::Spanned;
use syn::{Attribute, Fields, Meta, Type, TypePath, parse_str, visit::Visit};
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};

pub struct SysvarAccountDetector {
    diagnostics: Vec<Diagnostic>,
    config: DetectorConfig,
}

impl SysvarAccountDetector {
    pub fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
            config: DetectorConfig::default(),
        }
    }

    #[allow(dead_code)]
    pub fn with_config(config: DetectorConfig) -> Self {
        Self {
            diagnostics: Vec::new(),
            config,
        }
    }

    /// Check if a field is a Sysvar account type
    fn is_sysvar_field(&self, field: &syn::Field) -> Option<String> {
        if let Type::Path(TypePath { path, .. }) = &field.ty {
            if let Some(segment) = path.segments.last() {
                if segment.ident == "Sysvar" {
                    // Extract the sysvar type from generic arguments
                    if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                        // Look for the second generic argument (the sysvar type)
                        if args.args.len() >= 2 {
                            if let Some(syn::GenericArgument::Type(Type::Path(type_path))) =
                                args.args.iter().nth(1)
                            {
                                if let Some(type_segment) = type_path.path.segments.last() {
                                    let sysvar_type = type_segment.ident.to_string();
                                    // All sysvars support get() through the Sysvar trait
                                    return Some(sysvar_type);
                                }
                            }
                        }
                    }
                }
            }
        }
        None
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

    /// Generate suggestion message for the specific sysvar type
    fn get_suggestion_message(&self, sysvar_type: &str) -> String {
        format!(
            "Consider using {}::get()? instead of Sysvar<'info, {}>. The get() method is more efficient as it doesn't require passing the sysvar account in the transaction.",
            sysvar_type, sysvar_type
        )
    }
}

impl Detector for SysvarAccountDetector {
    fn id(&self) -> &'static str {
        "INEFFICIENT_SYSVAR_ACCOUNT"
    }

    fn name(&self) -> &'static str {
        "Inefficient Sysvar Account Usage"
    }

    fn description(&self) -> &'static str {
        "Detects usage of Sysvar<'info, Type> accounts and suggests using the more efficient get() method"
    }

    fn message(&self) -> &'static str {
        "Sysvar account usage detected. Consider using the get() method for better efficiency."
    }

    fn default_severity(&self) -> DiagnosticSeverity {
        DiagnosticSeverity::WARNING
    }

    fn analyze(&mut self, content: &str) -> Vec<Diagnostic> {
        self.diagnostics.clear();

        // Run default detection logic
        if let Ok(syntax_tree) = parse_str::<syn::File>(content) {
            self.visit_file(&syntax_tree);
        }

        self.diagnostics.clone()
    }
}

impl<'ast> Visit<'ast> for SysvarAccountDetector {
    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        // Check if this struct has #[derive(Accounts)]
        if !self.has_accounts_derive(&node.attrs) {
            return;
        }

        // Check each field for Sysvar usage
        if let Fields::Named(fields) = &node.fields {
            for field in &fields.named {
                if let Some(sysvar_type) = self.is_sysvar_field(field) {
                    let severity = self
                        .config
                        .severity_override
                        .unwrap_or(self.default_severity());

                    let message = self.get_suggestion_message(&sysvar_type);

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

        // Continue visiting children
        syn::visit::visit_item_struct(self, node);
    }
}
