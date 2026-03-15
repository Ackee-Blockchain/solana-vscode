#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use rustc_hir::def::Res;
use rustc_hir::def_id::DefId;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{
    BorrowKind, Expr, ExprKind, FnDecl, GenericArg, Item, ItemKind, Mutability, QPath, TyKind,
};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::hir::nested_filter;
use rustc_span::{Span, Symbol};
use std::collections::HashMap;

dylint_linting::declare_late_lint! {
    /// ### What it does
    /// Detects attempts to mutate Anchor accounts that are not marked as mutable
    /// with `#[account(mut)]`.
    ///
    /// ### Why is this bad?
    /// In Solana programs, attempting to mutate an account that is not declared as mutable
    /// will cause a runtime error:
    /// - The Solana runtime rejects transactions that modify accounts not marked as writable
    /// - Missing `#[account(mut)]` means changes will be silently discarded or cause failure
    /// - This is a common source of bugs in Anchor programs
    ///
    /// ### Example
    ///
    /// Bad:
    /// ```rust
    /// #[derive(Accounts)]
    /// pub struct UpdateVault<'info> {
    ///     pub vault: Account<'info, Vault>,
    /// }
    ///
    /// pub fn update_vault(ctx: Context<UpdateVault>, amount: u64) -> Result<()> {
    ///     ctx.accounts.vault.amount = amount; // Mutation of immutable account!
    ///     Ok(())
    /// }
    /// ```
    ///
    /// Good:
    /// ```rust
    /// #[derive(Accounts)]
    /// pub struct UpdateVault<'info> {
    ///     #[account(mut)]
    ///     pub vault: Account<'info, Vault>,
    /// }
    ///
    /// pub fn update_vault(ctx: Context<UpdateVault>, amount: u64) -> Result<()> {
    ///     ctx.accounts.vault.amount = amount;
    ///     Ok(())
    /// }
    /// ```
    pub IMMUTABLE_ACCOUNT_MUTATED,
    Warn,
    "detects attempts to mutate accounts not marked as mutable with #[account(mut)]"
}

/// Names of mutating methods on Solana accounts
const MUTATING_METHODS: &[&str] = &[
    "set_lamports",
    "set_data",
    "set_owner",
    "set_executable",
    "close",
    "realloc",
    "assign",
    "push",
    "insert",
    "remove",
    "clear",
    "set",
    "replace",
    "extend",
    "append",
    "truncate",
    "resize",
    "retain",
    "swap",
    "sort",
    "rotate",
    "fill",
];

/// Names of account types in Anchor that represent on-chain accounts
const ACCOUNT_TYPES: &[&str] = &["Account", "AccountInfo", "AccountLoader"];

impl<'tcx> LateLintPass<'tcx> for ImmutableAccountMutated {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        if let ItemKind::Fn { sig, body: body_id, .. } = item.kind {
            let def_id = item.owner_id.to_def_id();
            let visibility = cx.tcx.visibility(def_id);
            if !visibility.is_public() {
                return;
            }

            // Extract the Accounts struct type T from Context<T>
            let Some(context_def_id) = extract_context_type(cx, sig.decl) else {
                return;
            };

            // Find immutable account fields in the struct
            let immutable_fields = find_immutable_account_fields(cx, context_def_id);
            if immutable_fields.is_empty() {
                return;
            }

            // Walk the function body looking for mutations of immutable accounts
            let body = cx.tcx.hir_body(body_id);
            let mut visitor = MutationVisitor {
                cx,
                immutable_fields: &immutable_fields,
            };
            visitor.visit_expr(body.value);
        }
    }
}

/// Extract the DefId of type T from a `Context<T>` parameter
fn extract_context_type<'tcx>(cx: &LateContext<'tcx>, decl: &FnDecl<'tcx>) -> Option<DefId> {
    for param in decl.inputs {
        if let TyKind::Path(QPath::Resolved(None, path)) = &param.kind {
            if let Res::Def(_, def_id) = path.res {
                let type_name = cx.tcx.item_name(def_id);
                if type_name.as_str() != "Context" {
                    continue;
                }
                if let Some(segment) = path.segments.last() {
                    if let Some(args) = segment.args {
                        for arg in args.args {
                            if let GenericArg::Type(inner_ty) = arg {
                                if let TyKind::Path(QPath::Resolved(None, inner_path)) =
                                    &inner_ty.kind
                                {
                                    if let Res::Def(_, inner_def_id) = inner_path.res {
                                        return Some(inner_def_id);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Find fields in an Accounts struct that are account types without `#[account(mut)]` or `#[account(init)]`.
/// Returns a map of field name to field declaration span.
fn find_immutable_account_fields(cx: &LateContext<'_>, def_id: DefId) -> HashMap<Symbol, Span> {
    let mut immutable = HashMap::new();
    let tcx = cx.tcx;
    let adt_def = tcx.adt_def(def_id);

    for variant in adt_def.variants() {
        for field in &variant.fields {
            let field_ty = tcx.type_of(field.did).instantiate_identity();

            // Check if this is an account type (Account, AccountInfo, AccountLoader)
            if !is_account_type(cx, field_ty) {
                continue;
            }

            // Check if the field has #[account(mut)] or #[account(init)]
            if has_mut_or_init_attr(cx, field.did) {
                continue;
            }

            let field_span = tcx.def_span(field.did);
            immutable.insert(field.name, field_span);
        }
    }

    immutable
}

/// Check if a type is one of the Anchor account types
fn is_account_type(cx: &LateContext<'_>, ty: rustc_middle::ty::Ty<'_>) -> bool {
    if let rustc_middle::ty::TyKind::Adt(adt_def, _) = ty.kind() {
        let type_name = cx.tcx.item_name(adt_def.did());
        return ACCOUNT_TYPES.contains(&type_name.as_str());
    }
    false
}

/// Check if a field has `#[account(mut)]` or `#[account(init)]` attribute
fn has_mut_or_init_attr(cx: &LateContext<'_>, field_def_id: DefId) -> bool {
    let account_sym = Symbol::intern("account");
    let attrs = cx.tcx.get_attrs(field_def_id, account_sym);
    for attr in attrs {
        // Parse the attribute token stream for `mut` or `init` tokens
        let attr_str = format!("{:?}", attr);
        if attr_str.contains("mut") || attr_str.contains("init") {
            return true;
        }
    }
    false
}

/// Visitor that walks a function body looking for mutation attempts on immutable accounts
struct MutationVisitor<'a, 'tcx> {
    cx: &'a LateContext<'tcx>,
    immutable_fields: &'a HashMap<Symbol, Span>,
}

impl<'a, 'tcx> Visitor<'tcx> for MutationVisitor<'a, 'tcx> {
    type NestedFilter = nested_filter::OnlyBodies;

    fn maybe_tcx(&mut self) -> Self::MaybeTyCtxt {
        self.cx.tcx
    }

    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        match &expr.kind {
            // Direct assignment: ctx.accounts.vault.field = value
            ExprKind::Assign(lhs, _rhs, _span) => {
                if let Some(account_name) = self.expr_references_immutable_account(lhs) {
                    self.report(expr, account_name);
                }
            }

            // Compound assignment: ctx.accounts.vault.field += value
            ExprKind::AssignOp(_op, lhs, _rhs) => {
                if let Some(account_name) = self.expr_references_immutable_account(lhs) {
                    self.report(expr, account_name);
                }
            }

            // Mutable borrow: &mut ctx.accounts.vault
            ExprKind::AddrOf(BorrowKind::Ref, Mutability::Mut, inner) => {
                if let Some(account_name) = self.expr_references_immutable_account(inner) {
                    self.report(expr, account_name);
                }
            }

            // Method call: ctx.accounts.vault.set_lamports(...), etc.
            ExprKind::MethodCall(method, receiver, _args, _span) => {
                let method_name = method.ident.as_str();
                let is_mutating = MUTATING_METHODS
                    .iter()
                    .any(|m| method_name == *m || method_name.starts_with(m));

                if is_mutating {
                    if let Some(account_name) = self.expr_references_immutable_account(receiver) {
                        self.report(expr, account_name);
                    }
                }
            }

            _ => {}
        }

        intravisit::walk_expr(self, expr);
    }
}

impl<'a, 'tcx> MutationVisitor<'a, 'tcx> {
    /// Check if an expression references an immutable account field through `ctx.accounts.<field>`.
    /// Returns the field name if it does.
    fn expr_references_immutable_account(&self, expr: &Expr<'_>) -> Option<Symbol> {
        // Walk down field access chains to find `<something>.accounts.<field_name>.<...>`
        self.find_account_field_access(expr)
    }

    /// Recursively walk field access expressions to find an immutable account reference.
    /// Matches patterns like:
    ///   ctx.accounts.<field>
    ///   ctx.accounts.<field>.something
    ///   ctx.accounts.<field>.something.something_else
    fn find_account_field_access(&self, expr: &Expr<'_>) -> Option<Symbol> {
        match &expr.kind {
            ExprKind::Field(base, field_ident) => {
                // Check if this is `<base>.accounts.<field_name>` where field_name is immutable
                if self.immutable_fields.contains_key(&field_ident.name) {
                    if self.is_accounts_access(base) {
                        return Some(field_ident.name);
                    }
                }
                // Otherwise recurse into the base (handles chained field access)
                self.find_account_field_access(base)
            }
            // Handle method calls on account fields: ctx.accounts.vault.to_account_info()
            ExprKind::MethodCall(_method, receiver, _args, _span) => {
                self.find_account_field_access(receiver)
            }
            // Handle index expressions: ctx.accounts.vault.data[i]
            ExprKind::Index(base, _idx, _span) => self.find_account_field_access(base),
            // Handle unary deref: *ctx.accounts.vault
            ExprKind::Unary(rustc_hir::UnOp::Deref, inner) => {
                self.find_account_field_access(inner)
            }
            _ => None,
        }
    }

    /// Check if an expression is accessing the `accounts` field (e.g., `ctx.accounts`)
    /// Handles auto-deref chains that the compiler inserts (e.g., `Deref(Field(ctx, "accounts"))`)
    fn is_accounts_access(&self, expr: &Expr<'_>) -> bool {
        match &expr.kind {
            ExprKind::Field(_base, field_ident) => field_ident.name.as_str() == "accounts",
            // Auto-deref: the compiler inserts explicit Deref nodes for &T field access
            ExprKind::Unary(rustc_hir::UnOp::Deref, inner) => self.is_accounts_access(inner),
            _ => false,
        }
    }

    /// Report lint warnings: one on the mutation site, one on the field declaration
    fn report(&self, expr: &'tcx Expr<'tcx>, account_name: Symbol) {
        // Warning on the mutation site with link to field declaration
        let field_span = self.immutable_fields.get(&account_name).copied();
        clippy_utils::diagnostics::span_lint_and_then(
            self.cx,
            IMMUTABLE_ACCOUNT_MUTATED,
            expr.span,
            format!(
                "Attempting to mutate immutable account '{}'. Add #[account(mut)] to allow mutation.",
                account_name
            ),
            |diag| {
                if let Some(decl_span) = field_span {
                    diag.span_note(
                        decl_span,
                        format!("'{}' is declared here without #[account(mut)]", account_name),
                    );
                }
                diag.help(format!(
                    "add #[account(mut)] to the '{}' field in the Accounts struct",
                    account_name
                ));
            },
        );

        // Warning on the field declaration with a note navigating to the mutation site
        if let Some(&field_span) = self.immutable_fields.get(&account_name) {
            clippy_utils::diagnostics::span_lint_and_then(
                self.cx,
                IMMUTABLE_ACCOUNT_MUTATED,
                field_span,
                format!(
                    "Account '{}' is missing #[account(mut)] attribute",
                    account_name
                ),
                |diag| {
                    diag.span_note(
                        expr.span,
                        format!("'{}' is mutated here but not marked as mutable", account_name),
                    );
                    diag.help(format!(
                        "add #[account(mut)] above this field: #[account(mut)]\n    pub {}: ...",
                        account_name
                    ));
                },
            );
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
