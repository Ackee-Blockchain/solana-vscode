use super::detector::Detector;
use super::detector_config::DetectorConfig;
use crate::core::utilities::{DiagnosticBuilder, anchor_patterns::AnchorPatterns};
use std::collections::HashMap;
use std::collections::HashSet;
use syn::spanned::Spanned;
use syn::{Fields, parse_str, visit::Visit};
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};

#[derive(Default)]
pub struct ImmutableAccountMutatedDetector {
    diagnostics: Vec<Diagnostic>,
    config: DetectorConfig,
    current_context: Option<String>,
    context_immutable_accounts: HashMap<String, HashSet<String>>,
}

impl ImmutableAccountMutatedDetector {
    #[allow(dead_code)]
    pub fn with_config(config: DetectorConfig) -> Self {
        Self {
            diagnostics: Vec::new(),
            config,
            current_context: None,
            context_immutable_accounts: HashMap::new(),
        }
    }

    /// Check if a field is marked as immutable (no #[account(mut)] or #[account(init)] attribute)
    fn is_immutable_account_field(&self, field: &syn::Field) -> Option<String> {
        // Check if this field has a type that looks like an account
        let is_account_type = match &field.ty {
            syn::Type::Path(type_path) => {
                if let Some(segment) = type_path.path.segments.last() {
                    matches!(
                        segment.ident.to_string().as_str(),
                        "Account" | "AccountInfo" | "AccountLoader"
                    )
                } else {
                    false
                }
            }
            _ => false,
        };

        if !is_account_type {
            return None;
        }

        // Check if the field has either #[account(mut)] or #[account(init, ...)] attribute
        let has_mut_or_init = field.attrs.iter().any(|attr| {
            if attr.path().is_ident("account") {
                if let syn::Meta::List(meta_list) = &attr.meta {
                    let tokens = meta_list.tokens.to_string();
                    // Check for mut or init at word boundaries to avoid false positives
                    return tokens.split(',').any(|token| {
                        let token = token.trim();
                        token == "mut" || token.starts_with("init")
                    });
                }
            }
            false
        });

        // If it's an account type but doesn't have mut or init, it's immutable
        if !has_mut_or_init {
            field
                .ident
                .as_ref()
                .map(|field_name| field_name.to_string())
        } else {
            None
        }
    }

    /// Collect immutable accounts from an Accounts struct
    fn collect_immutable_accounts(
        &mut self,
        context_name: &str,
        accounts_struct: &syn::ItemStruct,
    ) {
        let mut immutable_accounts = HashSet::new();

        if let Fields::Named(fields) = &accounts_struct.fields {
            for field in &fields.named {
                if let Some(account_name) = self.is_immutable_account_field(field) {
                    immutable_accounts.insert(account_name);
                }
            }
        }

        self.context_immutable_accounts
            .insert(context_name.to_string(), immutable_accounts);
    }

    /// Check if an expression attempts to mutate an account
    fn is_mutation_attempt(&self, expr: &syn::Expr, account_name: &str) -> bool {
        match expr {
            // Direct assignment: account.field = value
            syn::Expr::Assign(assign_expr) => {
                self.expression_references_account(&assign_expr.left, account_name)
            }
            // Binary operations that could include compound assignment
            syn::Expr::Binary(binary_expr) => {
                // Check for compound assignment operators
                matches!(
                    binary_expr.op,
                    syn::BinOp::AddAssign(_)
                        | syn::BinOp::SubAssign(_)
                        | syn::BinOp::MulAssign(_)
                        | syn::BinOp::DivAssign(_)
                        | syn::BinOp::RemAssign(_)
                        | syn::BinOp::BitAndAssign(_)
                        | syn::BinOp::BitOrAssign(_)
                        | syn::BinOp::BitXorAssign(_)
                        | syn::BinOp::ShlAssign(_)
                        | syn::BinOp::ShrAssign(_)
                ) && self.expression_references_account(&binary_expr.left, account_name)
            }
            // Method calls that might mutate: account.method()
            syn::Expr::MethodCall(method_call) => {
                if self.expression_references_account(&method_call.receiver, account_name) {
                    let method_name = method_call.method.to_string();
                    // Check for common mutating methods
                    matches!(
                        method_name.as_str(),
                        "set_data"
                            | "set_lamports"
                            | "set_owner"
                            | "set_executable"
                            | "close"
                            | "realloc"
                            | "assign"
                    )
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Check if an expression references a specific account
    fn expression_references_account(&self, expr: &syn::Expr, account_name: &str) -> bool {
        match expr {
            syn::Expr::Path(path_expr) => {
                // Check if the path starts with the account name
                if let Some(first_segment) = path_expr.path.segments.first() {
                    first_segment.ident == account_name
                } else {
                    false
                }
            }
            syn::Expr::Field(field_expr) => {
                // Check if this field access is directly to our account name
                if let syn::Member::Named(field_name) = &field_expr.member {
                    if field_name == account_name {
                        // If it matches our account name, verify it's accessed through ctx.accounts
                        return self.is_accounts_access(&field_expr.base);
                    }
                }

                // If not a direct match, recursively check the base expression
                self.expression_references_account(&field_expr.base, account_name)
            }
            syn::Expr::MethodCall(method_call) => {
                self.expression_references_account(&method_call.receiver, account_name)
            }
            syn::Expr::Reference(ref_expr) => {
                self.expression_references_account(&ref_expr.expr, account_name)
            }
            syn::Expr::Unary(unary_expr) => {
                if matches!(unary_expr.op, syn::UnOp::Deref(_)) {
                    self.expression_references_account(&unary_expr.expr, account_name)
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Check if expression is accessing the "accounts" field (like ctx.accounts)
    fn is_accounts_access(&self, expr: &syn::Expr) -> bool {
        match expr {
            syn::Expr::Field(field_expr) => {
                if let syn::Member::Named(field_name) = &field_expr.member {
                    // Just check if we're accessing a field named "accounts"
                    // The parent context validation is handled by the Anchor framework
                    field_name == "accounts"
                } else {
                    false
                }
            }
            _ => false,
        }
    }
}

impl Detector for ImmutableAccountMutatedDetector {
    fn id(&self) -> &'static str {
        "IMMUTABLE_ACCOUNT_MUTATED"
    }

    fn name(&self) -> &'static str {
        "Immutable Account Mutation"
    }

    fn description(&self) -> &'static str {
        "Detects attempts to mutate accounts that are not marked as mutable with #[account(mut)]"
    }

    fn message(&self) -> &'static str {
        "Attempting to mutate an immutable account. Add #[account(mut)] to the account field to allow mutation."
    }

    fn default_severity(&self) -> DiagnosticSeverity {
        DiagnosticSeverity::ERROR
    }

    fn analyze(&mut self, content: &str) -> Vec<Diagnostic> {
        self.diagnostics.clear();
        self.context_immutable_accounts.clear();
        self.current_context = None;

        if let Ok(syntax_tree) = parse_str::<syn::File>(content) {
            // First pass: collect all immutable accounts for each context
            for item in &syntax_tree.items {
                if let syn::Item::Struct(item_struct) = item {
                    if AnchorPatterns::is_accounts_struct(item_struct) {
                        self.collect_immutable_accounts(
                            &item_struct.ident.to_string(),
                            item_struct,
                        );
                    }
                }
            }

            // Second pass: check for mutations in each context
            self.visit_file(&syntax_tree);
        }

        self.diagnostics.clone()
    }
}

impl<'ast> Visit<'ast> for ImmutableAccountMutatedDetector {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        // Check if this is an instruction function by looking at its first parameter
        if let Some(syn::FnArg::Typed(pat_type)) = node.sig.inputs.first() {
            if let syn::Type::Path(type_path) = &*pat_type.ty {
                if let Some(syn::PathSegment {
                    ident,
                    arguments: syn::PathArguments::AngleBracketed(args),
                }) = type_path.path.segments.first()
                {
                    if ident == "Context" {
                        if let Some(syn::GenericArgument::Type(syn::Type::Path(context_type))) =
                            args.args.first()
                        {
                            if let Some(type_segment) = context_type.path.segments.first() {
                                // Set the current context to the Accounts struct name
                                self.current_context = Some(type_segment.ident.to_string());
                            }
                        }
                    }
                }
            }
        }

        // Visit the function body
        syn::visit::visit_item_fn(self, node);

        // Clear the context after visiting the function
        self.current_context = None;
    }

    fn visit_expr(&mut self, node: &'ast syn::Expr) {
        // Only check for mutations if we're in a context
        if let Some(ref context) = self.current_context {
            if let Some(immutable_accounts) = self.context_immutable_accounts.get(context) {
                for account_name in immutable_accounts {
                    if self.is_mutation_attempt(node, account_name) {
                        log::debug!(
                            "Found mutation attempt for account: {} in context: {}",
                            account_name,
                            context
                        );
                        let severity = self
                            .config
                            .severity_override
                            .unwrap_or(self.default_severity());

                        let message = format!(
                            "Attempting to mutate immutable account '{}'. Add #[account(mut)] to allow mutation.",
                            account_name
                        );

                        self.diagnostics.push(DiagnosticBuilder::create(
                            DiagnosticBuilder::create_range_from_span(node.span()),
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
        syn::visit::visit_expr(self, node);
    }
}
