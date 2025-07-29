use super::detector::Detector;
use super::detector_config::DetectorConfig;
use crate::core::utilities::{DiagnosticBuilder, anchor_patterns::AnchorPatterns};
use log::{debug, info, warn};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use syn::spanned::Spanned;
use syn::{parse_str, visit::Visit};
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Range};

#[derive(Default)]
pub struct MissingSignerDetector {
    diagnostics: Vec<Diagnostic>,
    config: DetectorConfig,
    // Store context name and its location
    contexts: HashMap<String, Range>,
    // Track which contexts have signer fields
    context_has_signer: HashMap<String, bool>,
    // Track composite contexts and their components
    composite_contexts: HashMap<String, Vec<String>>,
    // Track which contexts we've already processed to avoid cycles
    processed_contexts: HashSet<String>,
    file_path: Option<PathBuf>,
    workspace_root: Option<PathBuf>,
}

impl MissingSignerDetector {
    #[allow(dead_code)]
    pub fn with_config(config: DetectorConfig) -> Self {
        Self {
            diagnostics: Vec::new(),
            config,
            contexts: HashMap::new(),
            context_has_signer: HashMap::new(),
            composite_contexts: HashMap::new(),
            processed_contexts: HashSet::new(),
            file_path: None,
            workspace_root: None,
        }
    }

    /// Check if a field is a Signer type
    fn is_signer_field(&self, field: &syn::Field) -> bool {
        AnchorPatterns::is_signer_field(field)
    }

    /// Check if a field is a composite account type (another Accounts struct)
    fn is_composite_field(&self, field: &syn::Field) -> Option<String> {
        if let syn::Type::Path(type_path) = &field.ty {
            if let Some(segment) = type_path.path.segments.last() {
                // Skip known Anchor types that aren't composite
                let name = segment.ident.to_string();
                if !matches!(
                    name.as_str(),
                    "Account"
                        | "AccountInfo"
                        | "UncheckedAccount"
                        | "Signer"
                        | "Program"
                        | "SystemProgram"
                        | "Clock"
                        | "Rent"
                        | "TokenAccount"
                ) {
                    return Some(name);
                }
            }
        }
        None
    }

    /// Check if an accounts struct has any signer fields, including nested ones
    fn check_accounts_struct(&mut self, struct_name: &str, item_struct: &syn::ItemStruct) -> bool {
        // Avoid infinite recursion by tracking processed contexts
        if self.processed_contexts.contains(struct_name) {
            // Return cached result if available
            return self
                .context_has_signer
                .get(struct_name)
                .copied()
                .unwrap_or(false);
        }

        // Mark as being processed
        self.processed_contexts.insert(struct_name.to_string());

        let mut has_direct_signer = false;
        let mut composite_fields = Vec::new();

        if let syn::Fields::Named(fields) = &item_struct.fields {
            for field in &fields.named {
                // Check for direct signer fields
                if self.is_signer_field(field) {
                    has_direct_signer = true;
                    break;
                }

                // Check for composite fields
                if let Some(composite_type) = self.is_composite_field(field) {
                    composite_fields.push(composite_type);
                }
            }
        }

        // If we found a direct signer, we're done
        if has_direct_signer {
            self.context_has_signer
                .insert(struct_name.to_string(), true);
            return true;
        }

        // Store composite relationships for later analysis
        if !composite_fields.is_empty() {
            self.composite_contexts
                .insert(struct_name.to_string(), composite_fields.clone());

            // Check if any of the composite fields have a signer (recursively if needed)
            if self.check_composite_fields_for_signer(&composite_fields) {
                self.context_has_signer
                    .insert(struct_name.to_string(), true);
                return true;
            }
        }

        // No direct or composite signer found
        self.context_has_signer
            .insert(struct_name.to_string(), false);
        false
    }

    /// Check if any of the composite fields have a signer (recursively if needed)
    fn check_composite_fields_for_signer(&mut self, composite_fields: &[String]) -> bool {
        for composite_type in composite_fields {
            // Check if we already know the result
            if let Some(has_signer) = self.context_has_signer.get(composite_type) {
                if *has_signer {
                    return true;
                }
            } else {
                // We don't know yet, so we need to find and check the struct
                // This will happen during the search_for_context_definitions phase
                // when we scan other files
            }

            // Check if this composite field itself has composite fields
            // Clone the nested fields to avoid borrowing issues
            let nested_fields_option = self.composite_contexts.get(composite_type).cloned();
            if let Some(nested_fields) = nested_fields_option {
                // Avoid cycles by checking if we've already processed this context
                if !self.processed_contexts.contains(composite_type) {
                    self.processed_contexts.insert(composite_type.clone());

                    // Recursively check nested composite fields
                    if self.check_composite_fields_for_signer(&nested_fields) {
                        self.context_has_signer.insert(composite_type.clone(), true);
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Search for context definitions in other files
    fn search_for_context_definitions(&mut self) {
        // Skip if we don't have a workspace root
        let Some(workspace_root) = &self.workspace_root else {
            warn!("No workspace root set, skipping context search");
            return;
        };

        // Skip if we don't have any contexts to search for
        if self.contexts.is_empty() {
            return;
        }

        // Get all contexts that we need to check, including composite components
        let mut contexts_to_find = HashSet::new();

        // Add direct contexts
        for ctx in self.contexts.keys() {
            contexts_to_find.insert(ctx.clone());
        }

        // Add composite components that we don't have info about yet
        for components in self.composite_contexts.values() {
            for component in components {
                if !self.context_has_signer.contains_key(component) {
                    contexts_to_find.insert(component.clone());
                }
            }
        }

        // Filter to only contexts we haven't found yet
        let missing_contexts: Vec<String> = contexts_to_find
            .into_iter()
            .filter(|ctx| !self.context_has_signer.contains_key(ctx))
            .collect();

        if missing_contexts.is_empty() {
            return;
        }

        info!("Searching for context definitions: {:?}", missing_contexts);

        // Walk through Rust files in the workspace
        if let Ok(rust_files) = self.walk_directory(workspace_root, &["rs"]) {
            for file_path in rust_files {
                // Skip the current file - we've already processed it
                if let Some(current_file) = &self.file_path {
                    if current_file == &file_path {
                        continue;
                    }
                }

                // Read and parse the file
                if let Ok(content) = fs::read_to_string(&file_path) {
                    if let Ok(syntax_tree) = parse_str::<syn::File>(&content) {
                        // Look for account structs that match our missing contexts
                        for item in &syntax_tree.items {
                            if let syn::Item::Struct(item_struct) = item {
                                if AnchorPatterns::is_accounts_struct(item_struct) {
                                    let struct_name = item_struct.ident.to_string();

                                    // Check if this is one of our missing contexts
                                    if missing_contexts.contains(&struct_name) {
                                        let has_signer =
                                            self.check_accounts_struct(&struct_name, item_struct);
                                        debug!(
                                            "Found external context definition: {} in {:?} with direct signer: {}",
                                            struct_name, file_path, has_signer
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Walk directory recursively to find files with given extensions
    fn walk_directory(
        &self,
        dir: &Path,
        extensions: &[&str],
    ) -> Result<Vec<PathBuf>, std::io::Error> {
        let mut files = Vec::new();
        self.walk_directory_recursive(dir, extensions, &mut files)?;
        Ok(files)
    }

    /// Recursive helper for walking directories
    fn walk_directory_recursive(
        &self,
        dir: &Path,
        extensions: &[&str],
        files: &mut Vec<PathBuf>,
    ) -> Result<(), std::io::Error> {
        if !dir.is_dir() {
            return Ok(());
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // Skip common directories that shouldn't be scanned
                if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
                    if matches!(
                        dir_name,
                        "target" | "node_modules" | ".git" | ".vscode" | "out"
                    ) {
                        continue;
                    }
                }
                self.walk_directory_recursive(&path, extensions, files)?;
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if extensions.contains(&ext) {
                    files.push(path);
                }
            }
        }

        Ok(())
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
        self.context_has_signer.clear();
        self.composite_contexts.clear();
        self.processed_contexts.clear();
        self.file_path = file_path.cloned();

        // If we don't have a workspace root yet, try to infer it from the file path
        if self.workspace_root.is_none() && file_path.is_some() {
            if let Some(parent) = file_path.and_then(|p| p.parent()) {
                // Try to find a reasonable workspace root (going up to 5 levels)
                let mut current = parent;
                for _ in 0..5 {
                    let cargo_toml = current.join("Cargo.toml");
                    if cargo_toml.exists() {
                        self.workspace_root = Some(current.to_path_buf());
                        break;
                    }
                    if let Some(next) = current.parent() {
                        current = next;
                    } else {
                        break;
                    }
                }
            }
        }

        info!("Starting Missing Signer analysis");

        // Basic parsing of the file
        if let Ok(syntax_tree) = parse_str::<syn::File>(content) {
            // First pass: find all account structs and check for signer fields
            self.visit_file_for_accounts(&syntax_tree);

            // Second pass: find all program functions and check their contexts
            self.visit_file(&syntax_tree);

            // Search for context definitions in other files
            self.search_for_context_definitions();

            // Generate diagnostics for contexts without signers
            self.generate_diagnostics();
        }

        self.diagnostics.clone()
    }
}

impl MissingSignerDetector {
    fn visit_file_for_accounts(&mut self, file: &syn::File) {
        for item in &file.items {
            if let syn::Item::Struct(item_struct) = item {
                if AnchorPatterns::is_accounts_struct(item_struct) {
                    let struct_name = item_struct.ident.to_string();
                    let has_signer = self.check_accounts_struct(&struct_name, item_struct);
                    debug!(
                        "Found accounts struct: {} with direct signer: {}",
                        struct_name, has_signer
                    );
                }
            }
        }
    }

    fn generate_diagnostics(&mut self) {
        for (context, range) in &self.contexts {
            // Check if we know about this context
            if let Some(has_signer) = self.context_has_signer.get(context) {
                if !*has_signer {
                    // This context has no signer field
                    let severity = self
                        .config
                        .severity_override
                        .unwrap_or(self.default_severity());

                    // Check if it's a composite context
                    if let Some(components) = self.composite_contexts.get(context) {
                        let components_str = components.join(", ");
                        self.diagnostics.push(DiagnosticBuilder::create(
                            *range,
                            format!(
                                "Context '{}' and its components ({}) have no signer field. Consider adding a Signer<'info> field to ensure proper authorization.",
                                context, components_str
                            ),
                            severity,
                            self.id().to_string(),
                            None,
                        ));
                    } else {
                        self.diagnostics.push(DiagnosticBuilder::create(
                            *range,
                            format!(
                                "Context '{}' has no signer field. Consider adding a Signer<'info> field to ensure proper authorization.",
                                context
                            ),
                            severity,
                            self.id().to_string(),
                            None,
                        ));
                    }
                }
            } else {
                debug!("Context not found: {}", context);
                // Context not found in any file - report it as a potential issue
                let severity = self
                    .config
                    .severity_override
                    .unwrap_or(self.default_severity());

                self.diagnostics.push(DiagnosticBuilder::create(
                    *range,
                    format!(
                        "Context '{}' definition not found. If it exists, ensure it has a Signer<'info> field for proper authorization.",
                        context
                    ),
                    severity,
                    self.id().to_string(),
                    None,
                ));
            }
        }
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
                    let context_name = inner_segment.ident.to_string();
                    debug!("Found inner segment Ident: {}", context_name);

                    // Store the context name and its location
                    let range = DiagnosticBuilder::create_range_from_span(inner_segment.span());
                    self.contexts.insert(context_name, range);
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
