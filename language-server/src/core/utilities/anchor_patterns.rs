// Anchor-specific AST patterns and utilities
pub struct AnchorPatterns;

impl AnchorPatterns {
    /// Check if a struct has the #[derive(Accounts)] attribute
    #[allow(dead_code)]
    pub fn is_accounts_struct(item_struct: &syn::ItemStruct) -> bool {
        item_struct.attrs.iter().any(|attr| {
            if let syn::Meta::List(meta_list) = &attr.meta {
                meta_list.path.is_ident("derive")
                    && meta_list.tokens.to_string().contains("Accounts")
            } else {
                false
            }
        })
    }

    /// Check if a function has the #[access_control] attribute
    #[allow(dead_code)]
    pub fn has_access_control(item_fn: &syn::ItemFn) -> bool {
        item_fn
            .attrs
            .iter()
            .any(|attr| attr.path().is_ident("access_control"))
    }



    /// Extract account constraints from field attributes
    #[allow(dead_code)]
    pub fn extract_account_constraints(field: &syn::Field) -> Vec<String> {
        let mut constraints = Vec::new();

        for attr in &field.attrs {
            if attr.path().is_ident("account") {
                if let syn::Meta::List(meta_list) = &attr.meta {
                    constraints.push(meta_list.tokens.to_string());
                }
            }
        }

        constraints
    }

    /// Check if a method call is a potential state modification
    #[allow(dead_code)]
    pub fn is_state_modifying_call(_method_call: &syn::ExprMethodCall) -> bool {
        // TODO: Add implementation
        false
    }

    /// Check if an expression contains security checks
    #[allow(dead_code)]
    pub fn has_security_check(expr: &syn::Expr) -> bool {
        match expr {
            syn::Expr::Macro(expr_macro) => {
                let macro_name = expr_macro
                    .mac
                    .path
                    .segments
                    .last()
                    .map(|seg| seg.ident.to_string())
                    .unwrap_or_default();
                matches!(
                    macro_name.as_str(),
                    "require" | "require_eq" | "require_keys_eq" | "require_neq"
                )
            }
            syn::Expr::MethodCall(method_call) => {
                let method_name = method_call.method.to_string();
                method_name.contains("has_one")
                    || method_name.contains("constraint")
                    || method_name.contains("owner")
                    || method_name.contains("authority")
                    || method_name.contains("signer")
            }
            _ => false,
        }
    }
}
