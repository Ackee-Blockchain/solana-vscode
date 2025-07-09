use super::detector::Detector;
use super::detector_config::DetectorConfig;
use crate::core::utilities::{DiagnosticBuilder, anchor_patterns::AnchorPatterns};
use std::path::PathBuf;
use syn::{parse_str, visit::Visit, Fields, Meta};
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};

#[derive(Default)]
pub struct InstructionAttributeUnusedDetector {
    diagnostics: Vec<Diagnostic>,
    config: DetectorConfig,
}

impl InstructionAttributeUnusedDetector {
    #[allow(dead_code)]
    pub fn with_config(config: DetectorConfig) -> Self {
        Self {
            diagnostics: Vec::new(),
            config,
        }
    }

    /// Check if a parameter is used in account constraints
    fn is_parameter_used_in_constraints(&self, param_name: &str, fields: &syn::FieldsNamed) -> bool {
        for field in &fields.named {
            // Check account constraints in field attributes
            for attr in &field.attrs {
                if attr.path().is_ident("account") {
                    if let Meta::List(meta_list) = &attr.meta {
                        let constraint_tokens = meta_list.tokens.to_string();
                        
                        // Check if parameter is referenced in constraints
                        if constraint_tokens.contains(param_name) {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    /// Check if parameter is used anywhere in the struct
    fn is_parameter_used(&self, param_name: &str, item_struct: &syn::ItemStruct) -> bool {
        if let Fields::Named(fields) = &item_struct.fields {
            return self.is_parameter_used_in_constraints(param_name, fields);
        }
        false
    }
}

impl Detector for InstructionAttributeUnusedDetector {
    fn id(&self) -> &'static str {
        "INSTRUCTION_ATTRIBUTE_UNUSED"
    }

    fn name(&self) -> &'static str {
        "Instruction Attribute Unused"
    }

    fn description(&self) -> &'static str {
        "Detects unused instruction parameters in the #[instruction(...)] attribute"
    }

    fn message(&self) -> &'static str {
        "Unused instruction parameter"
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

impl<'ast> Visit<'ast> for InstructionAttributeUnusedDetector {
    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        // Only check structs with #[derive(Accounts)]
        if !AnchorPatterns::is_accounts_struct(node) {
            return;
        }

        // Extract instruction parameters
        let instruction_params = AnchorPatterns::extract_instruction_parameters(node);
        
        // Check each parameter for usage
        for (param_name, _ ,param_span) in instruction_params {
            if !self.is_parameter_used(&param_name, node) {
                let severity = self
                    .config
                    .severity_override
                    .unwrap_or(self.default_severity());

                let message = format!("{}: '{}'", self.message(), param_name);

                self.diagnostics.push(DiagnosticBuilder::create(
                    DiagnosticBuilder::create_range_from_span(param_span),
                    message,
                    severity,
                    self.id().to_string(),
                    None,
                ));
            }
        }

        // Continue visiting children
        syn::visit::visit_item_struct(self, node);
    }
}
