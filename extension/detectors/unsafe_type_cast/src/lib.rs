#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_ast;
extern crate rustc_hir;
extern crate rustc_middle;

use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::{IntTy, TyKind, UintTy};

dylint_linting::declare_late_lint! {
    /// ### What it does
    /// Detects potentially unsafe integer type casts that could silently truncate values
    /// or produce unexpected results due to sign changes.
    ///
    /// ### Why is this bad?
    /// In Solana programs, narrowing casts (e.g., `u64` to `u32`) silently discard the
    /// upper bits, which can lead to:
    /// - Token amount manipulation
    /// - Incorrect balance calculations
    /// - Logic bugs from unexpected value wrapping
    ///
    /// Signed-to-unsigned casts (e.g., `i64` to `u64`) can turn negative values into
    /// large positive values, causing similar issues.
    ///
    /// ### Example
    ///
    /// Bad:
    /// ```rust
    /// let amount_u32 = amount_u64 as u32; // silently truncates
    /// let value = signed_val as u64;       // negative becomes large positive
    /// ```
    ///
    /// Good:
    /// ```rust
    /// let amount_u32: u32 = amount_u64.try_into().map_err(|_| ErrorCode::Overflow)?;
    /// let value: u64 = signed_val.try_into().map_err(|_| ErrorCode::InvalidValue)?;
    /// ```
    pub UNSAFE_TYPE_CAST,
    Warn,
    "detects potentially unsafe integer type casts that may truncate or change sign"
}

impl<'tcx> LateLintPass<'tcx> for UnsafeTypeCast {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        // Skip macro expansions
        if expr.span.from_expansion() {
            return;
        }

        let ExprKind::Cast(inner, _) = &expr.kind else {
            return;
        };

        // Skip small literal casts (e.g., `1 as u64`, `255 as u32`)
        if is_small_literal(inner) {
            return;
        }

        let source_ty = cx.typeck_results().expr_ty(inner);
        let target_ty = cx.typeck_results().expr_ty(expr);

        let Some(source_info) = IntTypeInfo::from_ty(source_ty.kind()) else {
            return;
        };
        let Some(target_info) = IntTypeInfo::from_ty(target_ty.kind()) else {
            return;
        };

        let is_narrowing = source_info.bit_width > target_info.bit_width;
        let is_signed_to_unsigned = source_info.is_signed && !target_info.is_signed;

        if is_narrowing {
            clippy_utils::diagnostics::span_lint_and_help(
                cx,
                UNSAFE_TYPE_CAST,
                expr.span,
                format!(
                    "unsafe cast from {} to {} may truncate value",
                    source_info.name, target_info.name
                ),
                None,
                "use `try_into()` or validate the value fits before casting",
            );
        } else if is_signed_to_unsigned {
            clippy_utils::diagnostics::span_lint_and_help(
                cx,
                UNSAFE_TYPE_CAST,
                expr.span,
                format!(
                    "unsafe cast from {} to {} may produce unexpected value for negative inputs",
                    source_info.name, target_info.name
                ),
                None,
                "use `try_into()` or validate the value is non-negative before casting",
            );
        }
    }
}

struct IntTypeInfo {
    name: &'static str,
    bit_width: u32,
    is_signed: bool,
}

impl IntTypeInfo {
    fn from_ty(ty: &TyKind<'_>) -> Option<Self> {
        // Treat isize/usize as 64-bit (Solana BPF target)
        match ty {
            TyKind::Int(int_ty) => {
                let (name, bit_width) = match int_ty {
                    IntTy::I8 => ("i8", 8),
                    IntTy::I16 => ("i16", 16),
                    IntTy::I32 => ("i32", 32),
                    IntTy::I64 => ("i64", 64),
                    IntTy::I128 => ("i128", 128),
                    IntTy::Isize => ("isize", 64),
                };
                Some(IntTypeInfo {
                    name,
                    bit_width,
                    is_signed: true,
                })
            }
            TyKind::Uint(uint_ty) => {
                let (name, bit_width) = match uint_ty {
                    UintTy::U8 => ("u8", 8),
                    UintTy::U16 => ("u16", 16),
                    UintTy::U32 => ("u32", 32),
                    UintTy::U64 => ("u64", 64),
                    UintTy::U128 => ("u128", 128),
                    UintTy::Usize => ("usize", 64),
                };
                Some(IntTypeInfo {
                    name,
                    bit_width,
                    is_signed: false,
                })
            }
            _ => None,
        }
    }
}

/// Check if an expression is a small literal that's unlikely to cause issues when cast
fn is_small_literal(expr: &Expr<'_>) -> bool {
    const SMALL_LITERAL_THRESHOLD: u128 = 1 << 16; // 65536

    match &expr.kind {
        ExprKind::Lit(lit) => match lit.node {
            rustc_ast::LitKind::Int(val, _) => val.get() < SMALL_LITERAL_THRESHOLD,
            rustc_ast::LitKind::Float(_, _) => true,
            _ => false,
        },
        ExprKind::Unary(rustc_hir::UnOp::Neg, inner) => is_small_literal(inner),
        ExprKind::Cast(inner, _) => is_small_literal(inner),
        _ => false,
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
