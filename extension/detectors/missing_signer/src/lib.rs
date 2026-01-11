#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use rustc_hir::def::Res;
use rustc_hir::def_id::DefId;
use rustc_hir::{FnDecl, FnSig, GenericArg, Item, ItemKind, QPath, Ty, TyKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::symbol::sym;
use std::collections::HashMap;

dylint_linting::declare_late_lint! {
    /// ### What it does
    /// Detects Anchor program instructions that have no signer accounts, which could allow
    /// unauthorized access.
    ///
    /// ### Why is this bad?
    /// In Solana programs, missing signer checks can lead to serious security vulnerabilities:
    /// - Anyone can call the instruction without authorization
    /// - Unauthorized users can modify program state
    /// - Attackers can drain funds or manipulate data
    /// - No way to verify who initiated the transaction
    ///
    /// ### Example
    ///
    /// Bad:
    /// ```rust
    /// #[derive(Accounts)]
    /// pub struct Transfer<'info> {
    ///     #[account(mut)]
    ///     pub from: Account<'info, TokenAccount>,
    ///     #[account(mut)]
    ///     pub to: Account<'info, TokenAccount>,
    /// }
    ///
    /// pub fn transfer(ctx: Context<Transfer>, amount: u64) -> Result<()> {
    ///     // No signer - anyone can transfer from any account!
    ///     Ok(())
    /// }
    /// ```
    ///
    /// Good:
    /// ```rust
    /// #[derive(Accounts)]
    /// pub struct Transfer<'info> {
    ///     pub authority: Signer<'info>,
    ///     #[account(mut)]
    ///     pub from: Account<'info, TokenAccount>,
    ///     #[account(mut)]
    ///     pub to: Account<'info, TokenAccount>,
    /// }
    ///
    /// pub fn transfer(ctx: Context<Transfer>, amount: u64) -> Result<()> {
    ///     // authority.key() can be used to verify ownership
    ///     Ok(())
    /// }
    /// ```
    pub MISSING_SIGNER,
    Warn,
    "detects Anchor program instructions with no signer accounts"
}

/// Stores information about Accounts structs and whether they have signers
struct AccountsAnalyzer {
    /// Maps struct DefId to whether it has a signer field
    has_signer: HashMap<DefId, bool>,
}

impl AccountsAnalyzer {
    fn new() -> Self {
        Self {
            has_signer: HashMap::new(),
        }
    }

    /// Check if a struct has a Signer field
    fn check_struct_has_signer(&mut self, cx: &LateContext<'_>, def_id: DefId) -> bool {
        // Check cache first
        if let Some(&cached) = self.has_signer.get(&def_id) {
            return cached;
        }

        let tcx = cx.tcx;
        let adt_def = tcx.adt_def(def_id);

        // Check all fields in the struct
        for variant in adt_def.variants() {
            for field in &variant.fields {
                let field_ty = tcx.type_of(field.did).instantiate_identity();

                // Check if this field is a Signer<'info>
                if self.is_signer_type(cx, field_ty) {
                    self.has_signer.insert(def_id, true);
                    return true;
                }

                // Check if this field is another Accounts struct (nested)
                if let Some(nested_def_id) = self.get_struct_def_id(field_ty) {
                    // Only check if it's an Accounts struct to avoid checking random nested types
                    if self.has_accounts_derive(cx, nested_def_id) {
                        // Recursively check the nested Accounts struct
                        if self.check_struct_has_signer(cx, nested_def_id) {
                            self.has_signer.insert(def_id, true);
                            return true;
                        }
                    }
                }
            }
        }

        self.has_signer.insert(def_id, false);
        false
    }

    /// Check if a type is Signer<'info>
    fn is_signer_type(&self, cx: &LateContext<'_>, ty: rustc_middle::ty::Ty<'_>) -> bool {
        if let rustc_middle::ty::TyKind::Adt(adt_def, _substs) = ty.kind() {
            let type_name = cx.tcx.item_name(adt_def.did());
            return type_name.as_str() == "Signer";
        }
        false
    }

    /// Get the DefId of a struct type
    fn get_struct_def_id(&self, ty: rustc_middle::ty::Ty<'_>) -> Option<DefId> {
        if let rustc_middle::ty::TyKind::Adt(adt_def, _) = ty.kind() {
            if adt_def.is_struct() {
                return Some(adt_def.did());
            }
        }
        None
    }

    /// Check if a struct has #[derive(Accounts)]
    fn has_accounts_derive(&self, cx: &LateContext<'_>, def_id: DefId) -> bool {
        let tcx = cx.tcx;
        let attrs = tcx.get_attrs(def_id, sym::derive);

        for attr in attrs {
            // Check if it derives Accounts
            let attr_str = format!("{:?}", attr);
            if attr_str.contains("Accounts") {
                return true;
            }
        }
        false
    }
}

impl<'tcx> LateLintPass<'tcx> for MissingSigner {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        // Only check public functions that take Context<T> parameter
        // We'll check the struct T when we find it's used in Context<T>
        if let ItemKind::Fn { sig, .. } = item.kind {
            let def_id = item.owner_id.to_def_id();
            let visibility = cx.tcx.visibility(def_id);
            if visibility.is_public() {
                self.check_program_function(cx, item, &sig);
            }
        }
    }
}

impl MissingSigner {
    /// Check a program function for missing signer
    fn check_program_function(&mut self, cx: &LateContext<'_>, item: &Item<'_>, sig: &FnSig<'_>) {
        // Look for Context<T> parameter
        if let Some(context_type_def_id) = self.extract_context_type(cx, sig.decl) {
            let mut analyzer = AccountsAnalyzer::new();

            // Check if the Accounts struct has a signer
            if !analyzer.check_struct_has_signer(cx, context_type_def_id) {
                // Report on the function (instruction level) - use ident span for just the function name
                let fn_span = sig.span;
                clippy_utils::diagnostics::span_lint_and_help(
                    cx,
                    MISSING_SIGNER,
                    fn_span,
                    "Instruction has no signer account",
                    None,
                    "Consider adding a Signer<'info> field to the accounts struct to ensure proper authorization",
                );

                // Also report on the struct definition itself
                if let Some(struct_span) = self.get_struct_span(cx, context_type_def_id) {
                    clippy_utils::diagnostics::span_lint_and_help(
                        cx,
                        MISSING_SIGNER,
                        struct_span,
                        "Accounts struct has no signer field",
                        None,
                        "Consider adding a Signer<'info> field to ensure proper authorization",
                    );
                }
            }
        }
    }

    /// Get the span of a struct definition from its DefId
    fn get_struct_span(&self, cx: &LateContext<'_>, def_id: DefId) -> Option<rustc_span::Span> {
        // Only works for local definitions (not external crates)
        if let Some(local_def_id) = def_id.as_local() {
            // Get the full definition span (entire struct declaration line including braces)
            return Some(cx.tcx.def_span(local_def_id));
        }
        None
    }

    /// Extract the Context<T> type from function parameters
    fn extract_context_type(&self, cx: &LateContext<'_>, decl: &FnDecl<'_>) -> Option<DefId> {
        for param in decl.inputs {
            if let Some(def_id) = self.get_context_generic_type(cx, param) {
                return Some(def_id);
            }
        }
        None
    }

    /// Get the generic type T from Context<T>
    fn get_context_generic_type(&self, cx: &LateContext<'_>, ty: &Ty<'_>) -> Option<DefId> {
        if let TyKind::Path(QPath::Resolved(None, path)) = &ty.kind {
            // Check if this is a Context type
            if let Res::Def(_, def_id) = path.res {
                let type_name = cx.tcx.item_name(def_id);
                if type_name.as_str() != "Context" {
                    return None;
                }

                // Extract the generic argument (the Accounts struct)
                if let Some(segment) = path.segments.last() {
                    if let Some(args) = segment.args {
                        for arg in args.args {
                            if let GenericArg::Type(inner_ty) = arg {
                                // Get the DefId of the inner type
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
        None
    }
}
