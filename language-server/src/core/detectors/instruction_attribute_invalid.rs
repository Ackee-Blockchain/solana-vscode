use super::detector::Detector;
use super::detector_config::DetectorConfig;
use crate::core::utilities::{DiagnosticBuilder, anchor_patterns::AnchorPatterns};
use std::path::PathBuf;
use syn::{parse_str, visit::Visit, Meta, ItemFn, FnArg, PatType, Type, TypePath};
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};
use std::collections::HashMap;

#[derive(Default)]
pub struct InstructionAttributeInvalidDetector {
    diagnostics: Vec<Diagnostic>,
    config: DetectorConfig,
    // Map from function name to its parameters (excluding Context)
    instruction_handlers: HashMap<String, Vec<(String, String)>>, // (param_name, param_type)
}

impl InstructionAttributeInvalidDetector {
    #[allow(dead_code)]
    pub fn with_config(config: DetectorConfig) -> Self {
        Self {
            diagnostics: Vec::new(),
            config,
            instruction_handlers: HashMap::new(),
        }
    }

    /// Extract parameters from an instruction handler function and return (context_struct_name, parameters)
    fn extract_handler_parameters(&self, item_fn: &ItemFn) -> Option<(String, Vec<(String, String)>)> {
        let mut parameters = Vec::new();
        let mut context_struct_name: Option<String> = None;
        
        for input in &item_fn.sig.inputs {
            if let FnArg::Typed(PatType { pat, ty, .. }) = input {
                if let syn::Pat::Ident(pat_ident) = &**pat {
                    let param_name = pat_ident.ident.to_string();
                    
                    // Check if this is a Context parameter
                    if let Type::Path(TypePath { path, .. }) = &**ty {
                        if let Some(segment) = path.segments.first() {
                            if segment.ident == "Context" {
                                // Extract the context struct name from Context<StructName>
                                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                                    if let Some(syn::GenericArgument::Type(Type::Path(context_type))) = args.args.first() {
                                        if let Some(context_segment) = context_type.path.segments.first() {
                                            context_struct_name = Some(context_segment.ident.to_string());
                                        }
                                    }
                                }
                                continue; // Skip the Context parameter itself
                            }
                        }
                    }
                    
                    // Extract type as string with better formatting
                    let type_str = self.extract_type_string(ty);
                    parameters.push((param_name, type_str));
                }
            }
        }
        
        // Only return if we found a context struct name
        context_struct_name.map(|struct_name| (struct_name, parameters))
    }

    /// Extract a clean type string from a syn::Type
    fn extract_type_string(&self, ty: &Type) -> String {
        match ty {
            Type::Path(type_path) => {
                let mut result = String::new();
                for (i, segment) in type_path.path.segments.iter().enumerate() {
                    if i > 0 {
                        result.push_str("::");
                    }
                    result.push_str(&segment.ident.to_string());
                    
                    // Handle generic arguments
                    if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                        result.push('<');
                        for (j, arg) in args.args.iter().enumerate() {
                            if j > 0 {
                                result.push_str(", ");
                            }
                            match arg {
                                syn::GenericArgument::Type(inner_ty) => {
                                    result.push_str(&self.extract_type_string(inner_ty));
                                }
                                syn::GenericArgument::Lifetime(lifetime) => {
                                    result.push_str(&format!("'{}", lifetime.ident));
                                }
                                _ => {
                                    result.push_str(&format!("{:?}", arg));
                                }
                            }
                        }
                        result.push('>');
                    }
                }
                result
            }
            Type::Reference(type_ref) => {
                let mut result = String::from("&");
                if let Some(lifetime) = &type_ref.lifetime {
                    result.push_str(&format!("'{} ", lifetime.ident));
                }
                if type_ref.mutability.is_some() {
                    result.push_str("mut ");
                }
                result.push_str(&self.extract_type_string(&type_ref.elem));
                result
            }
            Type::Slice(type_slice) => {
                format!("[{}]", self.extract_type_string(&type_slice.elem))
            }
            Type::Array(type_array) => {
                format!("[{}; {:?}]", self.extract_type_string(&type_array.elem), type_array.len)
            }
            Type::Tuple(type_tuple) => {
                let mut result = String::from("(");
                for (i, elem) in type_tuple.elems.iter().enumerate() {
                    if i > 0 {
                        result.push_str(", ");
                    }
                    result.push_str(&self.extract_type_string(elem));
                }
                result.push(')');
                result
            }
            _ => {
                // Fallback to debug format
                format!("{:?}", ty)
            }
        }
    }

    /// Check if instruction attribute parameters match the handler parameters
    fn validate_instruction_parameters(&self, struct_name: &str, instruction_params: &[(String, String, proc_macro2::Span)]) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        
        // Find the corresponding handler function using the exact struct name
        if let Some(handler_params) = self.instruction_handlers.get(struct_name) {
            
            // Check if instruction parameters are in the correct order and not skipping any
            for (i, (param_name, param_type, param_span)) in instruction_params.iter().enumerate() {
                if i >= handler_params.len() {
                    // Too many parameters in instruction attribute
                    let severity = self.config.severity_override.unwrap_or(self.default_severity());
                    let message = format!("Instruction parameter '{}' not found in handler function", param_name);
                    
                    diagnostics.push(DiagnosticBuilder::create(
                        DiagnosticBuilder::create_range_from_span(*param_span),
                        message,
                        severity,
                        self.id().to_string(),
                        None,
                    ));
                } else {
                    let (expected_name, expected_type) = &handler_params[i];
                    
                    // Check if parameter name matches
                    if param_name != expected_name {
                        let severity = self.config.severity_override.unwrap_or(self.default_severity());
                        let message = format!(
                            "Instruction parameter '{}' does not match handler parameter '{}' at position {}. Parameters must be in the same order as the handler function.",
                            param_name, expected_name, i + 1
                        );
                        
                        diagnostics.push(DiagnosticBuilder::create(
                            DiagnosticBuilder::create_range_from_span(*param_span),
                            message,
                            severity,
                            self.id().to_string(),
                            None,
                        ));
                    } else {
                        // Check if parameter type matches
                        let normalized_instruction_type = self.normalize_type(param_type);
                        let normalized_handler_type = self.normalize_type(expected_type);
                        
                        if normalized_instruction_type != normalized_handler_type {
                            let severity = self.config.severity_override.unwrap_or(self.default_severity());
                            let message = format!(
                                "Instruction parameter '{}' has type '{}' but handler function expects type '{}'",
                                param_name, param_type, expected_type
                            );
                            
                            diagnostics.push(DiagnosticBuilder::create(
                                DiagnosticBuilder::create_range_from_span(*param_span),
                                message,
                                severity,
                                self.id().to_string(),
                                None,
                            ));
                        }
                    }
                }
            }
        }
        
        diagnostics
    }

    /// Normalize type strings for comparison
    fn normalize_type(&self, type_str: &str) -> String {
        type_str
            .replace(" ", "")
            .replace("&str", "String") // Common alias
            .replace("&'static str", "String")
            .replace("&'_str", "String")
            .to_lowercase()
    }
}

impl Detector for InstructionAttributeInvalidDetector {
    fn id(&self) -> &'static str {
        "INSTRUCTION_ATTRIBUTE_INVALID"
    }

    fn name(&self) -> &'static str {
        "Instruction Attribute Invalid"
    }

    fn description(&self) -> &'static str {
        "Detects invalid use of instruction attribute - parameters must be in the same order as the handler function and cannot skip parameters"
    }

    fn message(&self) -> &'static str {
        "Invalid use of instruction attribute"
    }

    fn default_severity(&self) -> DiagnosticSeverity {
        DiagnosticSeverity::ERROR
    }

    fn analyze(&mut self, content: &str, _file_path: Option<&PathBuf>) -> Vec<Diagnostic> {
        self.diagnostics.clear();
        self.instruction_handlers.clear();

        // Run default detection logic
        if let Ok(syntax_tree) = parse_str::<syn::File>(content) {
            // Collect all instruction handler functions (any function with Context<T> parameter)
            self.visit_file(&syntax_tree);
        }

        self.diagnostics.clone()
    }
}

impl<'ast> Visit<'ast> for InstructionAttributeInvalidDetector {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        // Collect any function that has a Context<T> parameter (potential instruction handler)
        if let Some((context_struct_name, params)) = self.extract_handler_parameters(node) {
            self.instruction_handlers.insert(context_struct_name, params);
        }
        
        // Continue visiting children
        syn::visit::visit_item_fn(self, node);
    }

    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        // Only check structs with #[derive(Accounts)]
        if !AnchorPatterns::is_accounts_struct(node) {
            return;
        }

        // Extract instruction parameters
        let instruction_params = AnchorPatterns::extract_instruction_parameters(node);
        
        // Only validate if there are instruction parameters
        if !instruction_params.is_empty() {
            let struct_name = node.ident.to_string();
            let validation_diagnostics = self.validate_instruction_parameters(&struct_name, &instruction_params);
            self.diagnostics.extend(validation_diagnostics);
        }

        // Continue visiting children
        syn::visit::visit_item_struct(self, node);
    }
}
