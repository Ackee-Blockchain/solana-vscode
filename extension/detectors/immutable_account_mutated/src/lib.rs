#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use rustc_hir as hir;
use rustc_hir::def_id::DefId;
use rustc_hir::{Expr, ExprKind, FnDecl};
use rustc_lint::{LateContext, LateLintPass};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

dylint_linting::declare_late_lint! {
    /// ### What it does
    /// Detects attempts to mutate Anchor accounts that are not marked as mutable with `#[account(mut)]`.
    ///
    /// ### Why is this bad?
    /// In Solana/Anchor programs, attempting to mutate an immutable account will cause a runtime error:
    /// - The transaction will fail with "Account is not writable"
    /// - Wastes compute units and transaction fees
    /// - Can cause unexpected program failures
    ///
    /// ### Example
    ///
    /// Bad:
    /// ```rust
    /// #[derive(Accounts)]
    /// pub struct UpdateData<'info> {
    ///     pub data_account: Account<'info, DataAccount>,  // Missing #[account(mut)]
    /// }
    ///
    /// pub fn update(ctx: Context<UpdateData>) -> Result<()> {
    ///     ctx.accounts.data_account.value = 42;  // Error!
    ///     Ok(())
    /// }
    /// ```
    ///
    /// Good:
    /// ```rust
    /// #[derive(Accounts)]
    /// pub struct UpdateData<'info> {
    ///     #[account(mut)]
    ///     pub data_account: Account<'info, DataAccount>,
    /// }
    /// ```
    pub IMMUTABLE_ACCOUNT_MUTATED,
    Deny,
    "detects attempts to mutate immutable Anchor accounts"
}

// Thread-local storage for tracking immutable accounts across the lint
thread_local! {
    static IMMUTABLE_ACCOUNTS: RefCell<HashMap<DefId, HashSet<String>>> = RefCell::new(HashMap::new());
    static FIELD_SPANS: RefCell<HashMap<DefId, HashMap<String, rustc_span::Span>>> = RefCell::new(HashMap::new());
    static CURRENT_CONTEXT: RefCell<Option<DefId>> = RefCell::new(None);
}

impl<'tcx> LateLintPass<'tcx> for ImmutableAccountMutated {
    fn check_crate(&mut self, _cx: &LateContext<'tcx>) {
        // Clear thread-local storage for this crate
        IMMUTABLE_ACCOUNTS.with(|map| map.borrow_mut().clear());
        FIELD_SPANS.with(|map| map.borrow_mut().clear());
        CURRENT_CONTEXT.with(|ctx| *ctx.borrow_mut() = None);
    }

    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::Item<'tcx>) {
        // Collect immutable account info from structs
        if let hir::ItemKind::Struct(_, _, ref variant_data) = item.kind {
            let struct_def_id = item.owner_id.to_def_id();
            Self::collect_immutable_fields(cx, struct_def_id, variant_data);
        }
    }

    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        _: hir::intravisit::FnKind<'tcx>,
        _: &'tcx FnDecl<'tcx>,
        body: &'tcx hir::Body<'tcx>,
        _: rustc_span::Span,
        _: rustc_span::def_id::LocalDefId,
    ) {
        // Extract Context<T> and set it as current context
        if let Some(context_def_id) = Self::extract_context_from_body(cx, body) {
            CURRENT_CONTEXT.with(|ctx| *ctx.borrow_mut() = Some(context_def_id));
        }
    }

    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        // Only check if we're inside a function with Context parameter
        let context_def_id = CURRENT_CONTEXT.with(|ctx| *ctx.borrow());

        if let Some(ctx_id) = context_def_id {
            // Check for assignment mutations
            if let ExprKind::Assign(lhs, _, _) = expr.kind {
                if let Some(account_name) = Self::extract_account_name(lhs) {
                    Self::report_if_immutable(cx, &account_name, lhs.span, ctx_id);
                }
            }
        }
    }
}

impl ImmutableAccountMutated {
    /// Collect immutable account fields from a struct
    fn collect_immutable_fields(
        cx: &LateContext<'_>,
        struct_def_id: DefId,
        variant_data: &hir::VariantData<'_>,
    ) {
        let mut immutable_fields = HashSet::new();
        let mut field_spans = HashMap::new();

        for field in variant_data.fields() {
            let field_def_id = field.def_id.to_def_id();
            let field_name = cx.tcx.item_name(field_def_id).to_string();
            let field_ty = cx.typeck_results().node_type(field.hir_id);

            // Only check Account types
            if !Self::is_account_type(cx, field_ty) {
                continue;
            }

            // Store field span
            field_spans.insert(field_name.clone(), field.span);

            // Check if mutable by parsing source
            let is_mut = Self::check_field_is_mutable(cx, field);

            if !is_mut {
                immutable_fields.insert(field_name);
            }
        }

        if !immutable_fields.is_empty() {
            IMMUTABLE_ACCOUNTS.with(|map| {
                map.borrow_mut().insert(struct_def_id, immutable_fields);
            });
            FIELD_SPANS.with(|map| {
                map.borrow_mut().insert(struct_def_id, field_spans);
            });
        }
    }

    /// Check if a field is mutable by parsing its source
    fn check_field_is_mutable(cx: &LateContext<'_>, field: &hir::FieldDef<'_>) -> bool {
        if let Some(src) = clippy_utils::source::snippet_opt(cx, field.span) {
            // Look for #[account(mut)] or #[account(init, ...)]
            src.contains("#[account(mut)") || src.contains("#[account(init")
        } else {
            false
        }
    }

    /// Check if a type is Account, AccountInfo, or AccountLoader
    fn is_account_type(cx: &LateContext<'_>, ty: rustc_middle::ty::Ty<'_>) -> bool {
        if let rustc_middle::ty::TyKind::Adt(adt_def, _) = ty.kind() {
            let type_name = cx.tcx.item_name(adt_def.did());
            matches!(
                type_name.as_str(),
                "Account" | "AccountInfo" | "AccountLoader"
            )
        } else {
            false
        }
    }

    /// Extract Context<T> from function body parameters
    fn extract_context_from_body(cx: &LateContext<'_>, body: &hir::Body<'_>) -> Option<DefId> {
        for param in body.params {
            let param_ty = cx.typeck_results().node_type(param.hir_id);

            if let rustc_middle::ty::TyKind::Adt(adt_def, substs) = param_ty.kind() {
                let type_name = cx.tcx.item_name(adt_def.did());

                if type_name.as_str() == "Context" {
                    // Get the last generic (the Accounts struct, after lifetimes)
                    if let Some(last_generic) = substs.last() {
                        if let Some(generic_ty) = last_generic.as_type() {
                            if let rustc_middle::ty::TyKind::Adt(inner_adt, _) = generic_ty.kind() {
                                return Some(inner_adt.did());
                            }
                        }
                    }
                }
            }
        }
        None
    }

    /// Extract account name from ctx.accounts.account_name.field pattern
    fn extract_account_name(expr: &Expr<'_>) -> Option<String> {
        let mut field_chain = Vec::new();
        let mut current = expr;

        loop {
            match current.kind {
                ExprKind::Field(base, field) => {
                    field_chain.push(field.as_str().to_string());
                    current = base;
                }
                _ => break,
            }
        }

        field_chain.reverse();

        // Find "accounts" and return the next field
        for i in 0..field_chain.len() {
            if field_chain[i] == "accounts" && i + 1 < field_chain.len() {
                return Some(field_chain[i + 1].clone());
            }
        }

        None
    }

    /// Report if account is immutable
    fn report_if_immutable(
        cx: &LateContext<'_>,
        account_name: &str,
        span: rustc_span::Span,
        context_def_id: DefId,
    ) {
        let is_immutable = IMMUTABLE_ACCOUNTS.with(|map| {
            map.borrow()
                .get(&context_def_id)
                .map(|set| set.contains(account_name))
                .unwrap_or(false)
        });

        if is_immutable {
            // Report at mutation site
            clippy_utils::diagnostics::span_lint_and_help(
                cx,
                IMMUTABLE_ACCOUNT_MUTATED,
                span,
                format!("attempting to mutate immutable account '{}'", account_name),
                None,
                "add #[account(mut)] attribute to the account field",
            );

            // Report at field definition
            FIELD_SPANS.with(|map| {
                if let Some(fields) = map.borrow().get(&context_def_id) {
                    if let Some(&field_span) = fields.get(account_name) {
                        clippy_utils::diagnostics::span_lint_and_help(
                            cx,
                            IMMUTABLE_ACCOUNT_MUTATED,
                            field_span,
                            format!("account field '{}' is not marked as mutable", account_name),
                            None,
                            "add #[account(mut)] attribute here",
                        );
                    }
                }
            });
        }
    }
}
