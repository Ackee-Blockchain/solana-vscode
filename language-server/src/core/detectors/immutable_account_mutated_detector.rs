use super::detector::Detector;
use super::detector_config::DetectorConfig;
use crate::core::utilities::{DiagnosticBuilder, anchor_patterns::AnchorPatterns};
use std::collections::HashSet;
use syn::spanned::Spanned;
use syn::{Fields, parse_str, visit::Visit};
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};

pub struct ImmutableAccountMutatedDetector {
    diagnostics: Vec<Diagnostic>,
    config: DetectorConfig,
    immutable_accounts: HashSet<String>,
}

impl ImmutableAccountMutatedDetector {
    pub fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
            config: DetectorConfig::default(),
            immutable_accounts: HashSet::new(),
        }
    }

    #[allow(dead_code)]
    pub fn with_config(config: DetectorConfig) -> Self {
        Self {
            diagnostics: Vec::new(),
            config,
            immutable_accounts: HashSet::new(),
        }
    }

    /// Check if a field is marked as immutable (no #[account(mut)] attribute)
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

        // Check if the field has the #[account(mut)] attribute
        let has_mut_attribute = field.attrs.iter().any(|attr| {
            if attr.path().is_ident("account") {
                if let syn::Meta::List(meta_list) = &attr.meta {
                    return meta_list.tokens.to_string().contains("mut");
                }
            }
            false
        });

        // If it's an account type but doesn't have mut, it's immutable
        if !has_mut_attribute {
            if let Some(field_name) = field.ident.as_ref() {
                Some(field_name.to_string())
            } else {
                None
            }
        } else {
            None
        }
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
                // Recursively check the base expression and see if it references our account
                self.expression_references_account(&field_expr.base, account_name)
                    || self.check_nested_field_access(&field_expr.base, account_name)
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

    /// Check nested field access like ctx.accounts.account_name
    fn check_nested_field_access(&self, expr: &syn::Expr, account_name: &str) -> bool {
        match expr {
            syn::Expr::Field(field_expr) => {
                // Check if this is accessing accounts.<account_name>
                if let syn::Member::Named(field_name) = &field_expr.member {
                    if field_name == account_name {
                        // Check if the base is accessing "accounts"
                        return self.is_accounts_access(&field_expr.base);
                    }
                }
                // Continue checking recursively
                self.check_nested_field_access(&field_expr.base, account_name)
            }
            _ => false,
        }
    }

    /// Check if expression is accessing the "accounts" field (like ctx.accounts)
    fn is_accounts_access(&self, expr: &syn::Expr) -> bool {
        match expr {
            syn::Expr::Field(field_expr) => {
                if let syn::Member::Named(field_name) = &field_expr.member {
                    field_name == "accounts"
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Collect immutable account names from an Accounts struct
    fn collect_immutable_accounts(&mut self, accounts_struct: &syn::ItemStruct) {
        self.immutable_accounts.clear();

        if let Fields::Named(fields) = &accounts_struct.fields {
            for field in &fields.named {
                if let Some(account_name) = self.is_immutable_account_field(field) {
                    self.immutable_accounts.insert(account_name);
                }
            }
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
        self.immutable_accounts.clear();

        if let Ok(syntax_tree) = parse_str::<syn::File>(content) {
            self.visit_file(&syntax_tree);
        }

        self.diagnostics.clone()
    }
}

impl<'ast> Visit<'ast> for ImmutableAccountMutatedDetector {
    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        // Check if this struct has #[derive(Accounts)]
        if AnchorPatterns::is_accounts_struct(node) {
            println!("Found Accounts struct: {}", node.ident);
            self.collect_immutable_accounts(node);
            println!(
                "Collected {} immutable accounts",
                self.immutable_accounts.len()
            );
            for account in &self.immutable_accounts {
                println!("  - {}", account);
            }
        }

        // Continue visiting children
        syn::visit::visit_item_struct(self, node);
    }

    fn visit_expr(&mut self, node: &'ast syn::Expr) {
        // Check if this expression mutates any immutable accounts
        for account_name in &self.immutable_accounts.clone() {
            if self.is_mutation_attempt(node, account_name) {
                println!("Found mutation attempt for account: {}", account_name);
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

        // Continue visiting children
        syn::visit::visit_expr(self, node);
    }
}
