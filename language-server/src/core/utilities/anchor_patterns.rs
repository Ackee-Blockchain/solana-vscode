// Anchor-specific AST patterns and utilities
pub struct AnchorPatterns;

impl AnchorPatterns {
    /// Check if a struct has the #[derive(Accounts)] attribute
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

    /// Check if a struct has the #[account] attribute
    pub fn is_account_struct(item_struct: &syn::ItemStruct) -> bool {
        item_struct
            .attrs
            .iter()
            .any(|attr| attr.path().is_ident("account"))
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

    /// Extract parameter names and types from the #[instruction(...)] attribute
    ///
    /// Returns a vector of tuples, where each tuple contains:
    /// - The parameter name
    /// - The parameter type
    /// - The span of the parameter
    pub fn extract_instruction_parameters(
        item_struct: &syn::ItemStruct,
    ) -> Vec<(String, String, proc_macro2::Span)> {
        let mut parameters = Vec::new();

        for attr in &item_struct.attrs {
            if attr.path().is_ident("instruction") {
                if let syn::Meta::List(meta_list) = &attr.meta {
                    // Parse the tokens to get individual parameter spans
                    let tokens = &meta_list.tokens;
                    let mut token_iter = tokens.clone().into_iter();
                    let mut current_param = String::new();
                    let mut current_type = String::new();
                    let mut param_start_span: Option<proc_macro2::Span> = None;
                    let mut parsing_type = false;

                    while let Some(token) = token_iter.next() {
                        match token {
                            proc_macro2::TokenTree::Ident(ident) => {
                                if current_param.is_empty() && !parsing_type {
                                    // This is the start of a parameter name
                                    current_param = ident.to_string();
                                    param_start_span = Some(ident.span());
                                } else if parsing_type {
                                    // This is part of the type
                                    current_type.push_str(&ident.to_string());
                                }
                            }
                            proc_macro2::TokenTree::Punct(punct) => {
                                if punct.as_char() == ':'
                                    && !current_param.is_empty()
                                    && !parsing_type
                                {
                                    parsing_type = true;
                                } else if punct.as_char() == ',' && parsing_type {
                                    // End of this parameter
                                    if let Some(span) = param_start_span {
                                        parameters.push((
                                            current_param.clone(),
                                            current_type.trim().to_string(),
                                            span,
                                        ));
                                    }
                                    current_param.clear();
                                    current_type.clear();
                                    param_start_span = None;
                                    parsing_type = false;
                                } else if parsing_type {
                                    // Add punctuation to type (for things like &str, Vec<T>, etc.)
                                    current_type.push(punct.as_char());
                                }
                            }
                            proc_macro2::TokenTree::Group(group) => {
                                if parsing_type {
                                    // Add group content to type (for generics, etc.)
                                    let (open_char, close_char) = match group.delimiter() {
                                        proc_macro2::Delimiter::Parenthesis => ('(', ')'),
                                        proc_macro2::Delimiter::Brace => ('{', '}'),
                                        proc_macro2::Delimiter::Bracket => ('[', ']'),
                                        proc_macro2::Delimiter::None => (' ', ' '),
                                    };
                                    current_type.push_str(&format!(
                                        "{}{}{}",
                                        open_char,
                                        group.stream().to_string(),
                                        close_char
                                    ));
                                }
                            }
                            _ => {}
                        }
                    }

                    // Handle the last parameter if we ended without a comma
                    if !current_param.is_empty() && parsing_type {
                        if let Some(span) = param_start_span {
                            parameters.push((current_param, current_type.trim().to_string(), span));
                        }
                    }
                }
            }
        }

        parameters
    }

    /// Check if a field type is AccountInfo or UncheckedAccount
    pub fn is_unchecked_account_type(field: &syn::Field) -> Option<String> {
        if let syn::Type::Path(syn::TypePath { path, .. }) = &field.ty {
            if let Some(segment) = path.segments.last() {
                let type_name = segment.ident.to_string();
                if type_name == "AccountInfo" || type_name == "UncheckedAccount" {
                    return Some(type_name);
                }
            }
        }
        None
    }
}
