#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_ast;
extern crate rustc_hir;
extern crate rustc_middle;

use rustc_hir::{BinOpKind, Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::TyKind;

dylint_linting::declare_late_lint! {
    /// ### What it does
    /// Detects unchecked arithmetic operations (addition, subtraction, multiplication, division)
    /// that could lead to overflow or underflow vulnerabilities in Solana programs.
    ///
    /// ### Why is this bad?
    /// In Solana programs, arithmetic overflow/underflow can lead to serious security vulnerabilities:
    /// - Token amount manipulation
    /// - Incorrect balance calculations
    /// - Access control bypasses
    /// - Economic exploits
    ///
    /// ### Example
    ///
    /// Bad:
    /// ```rust
    /// let total = amount1 + amount2; // Could overflow
    /// let balance = balance - withdrawal; // Could underflow
    /// ```
    ///
    /// Good:
    /// ```rust
    /// let total = amount1.checked_add(amount2).ok_or(ErrorCode::Overflow)?;
    /// let balance = balance.checked_sub(withdrawal).ok_or(ErrorCode::Underflow)?;
    /// ```
    pub UNCHECKED_MATH,
    Warn,
    "detects unchecked arithmetic operations that could overflow/underflow"
}

impl<'tcx> LateLintPass<'tcx> for UncheckedMath {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        // Skip if inside a macro expansion to avoid false positives
        if expr.span.from_expansion() {
            return;
        }

        match expr.kind {
            // Check binary operations (+, -, *, /)
            ExprKind::Binary(op, left, right) => {
                if let Some((msg, help)) = check_arithmetic_op(cx, op.node, left, right, false) {
                    clippy_utils::diagnostics::span_lint_and_help(
                        cx,
                        UNCHECKED_MATH,
                        expr.span,
                        msg,
                        None,
                        help,
                    );
                }
            }
            // Check compound assignment operators (+=, -=, *=, /=)
            ExprKind::AssignOp(op, left, right) => {
                if let Some((msg, help)) = check_arithmetic_op(cx, op.node, left, right, true) {
                    clippy_utils::diagnostics::span_lint_and_help(
                        cx,
                        UNCHECKED_MATH,
                        expr.span,
                        msg,
                        None,
                        help,
                    );
                }
            }
            _ => {}
        }
    }
}

/// Check if an arithmetic operation is potentially unsafe and return appropriate message
fn check_arithmetic_op<'tcx>(
    cx: &LateContext<'tcx>,
    op: BinOpKind,
    left: &'tcx Expr<'tcx>,
    right: &'tcx Expr<'tcx>,
    is_assignment: bool,
) -> Option<(&'static str, &'static str)> {
    // Only check arithmetic operations
    if !matches!(
        op,
        BinOpKind::Add | BinOpKind::Sub | BinOpKind::Mul | BinOpKind::Div
    ) {
        return None;
    }

    // Check if the operation is on numeric types that could overflow
    if !is_potentially_unsafe_operation(cx, left, right) {
        return None;
    }

    // Return appropriate message based on operation type
    Some(get_lint_message(op, is_assignment))
}

/// Get the appropriate lint message for an operation
fn get_lint_message(op: BinOpKind, is_assignment: bool) -> (&'static str, &'static str) {
    match (op, is_assignment) {
        (BinOpKind::Add, false) => (
            "unchecked addition operation detected",
            "consider using `checked_add()` to prevent overflow/underflow",
        ),
        (BinOpKind::Add, true) => (
            "unchecked addition assignment operation detected",
            "consider using `checked_add()` and reassigning the result to prevent overflow/underflow",
        ),
        (BinOpKind::Sub, false) => (
            "unchecked subtraction operation detected",
            "consider using `checked_sub()` to prevent overflow/underflow",
        ),
        (BinOpKind::Sub, true) => (
            "unchecked subtraction assignment operation detected",
            "consider using `checked_sub()` and reassigning the result to prevent overflow/underflow",
        ),
        (BinOpKind::Mul, false) => (
            "unchecked multiplication operation detected",
            "consider using `checked_mul()` to prevent overflow/underflow",
        ),
        (BinOpKind::Mul, true) => (
            "unchecked multiplication assignment operation detected",
            "consider using `checked_mul()` and reassigning the result to prevent overflow/underflow",
        ),
        (BinOpKind::Div, false) => (
            "unchecked division operation detected",
            "consider using `checked_div()` to prevent overflow/underflow",
        ),
        (BinOpKind::Div, true) => (
            "unchecked division assignment operation detected",
            "consider using `checked_div()` and reassigning the result to prevent overflow/underflow",
        ),
        _ => unreachable!("Only arithmetic operations should reach here"),
    }
}

/// Check if an operation is potentially unsafe (could overflow/underflow)
fn is_potentially_unsafe_operation<'tcx>(
    cx: &LateContext<'tcx>,
    left: &'tcx Expr<'tcx>,
    right: &'tcx Expr<'tcx>,
) -> bool {
    use rustc_middle::ty::{IntTy, UintTy};

    // Get the types of the operands
    let left_ty = cx.typeck_results().expr_ty(left);
    let right_ty = cx.typeck_results().expr_ty(right);

    // Check if at least one operand is an integer type that could overflow
    // We check ALL integer types including u8, i8, u16, i16, etc.
    // Even small types like u8 can overflow (e.g., 255 + 1 = 0 in wrapping mode)
    let has_integer_type = matches!(
        left_ty.kind(),
        TyKind::Int(IntTy::I8 | IntTy::I16 | IntTy::I32 | IntTy::I64 | IntTy::I128 | IntTy::Isize)
            | TyKind::Uint(
                UintTy::U8 | UintTy::U16 | UintTy::U32 | UintTy::U64 | UintTy::U128 | UintTy::Usize
            )
    ) || matches!(
        right_ty.kind(),
        TyKind::Int(IntTy::I8 | IntTy::I16 | IntTy::I32 | IntTy::I64 | IntTy::I128 | IntTy::Isize)
            | TyKind::Uint(
                UintTy::U8 | UintTy::U16 | UintTy::U32 | UintTy::U64 | UintTy::U128 | UintTy::Usize
            )
    );

    if !has_integer_type {
        return false;
    }

    // Skip if both operands are small compile-time constants
    // These are unlikely to overflow and often used for array indexing, etc.
    if is_small_literal(left) && is_small_literal(right) {
        return false;
    }

    true
}

/// Check if an expression is a small literal that's unlikely to overflow
///
/// This helps reduce false positives for common patterns like:
/// - Array indexing: `arr[i + 1]`
/// - Small offsets: `value + 10`
/// - Common constants: `x * 2`, `y - 1`
fn is_small_literal(expr: &Expr<'_>) -> bool {
    const SMALL_LITERAL_THRESHOLD: u128 = 1 << 16; // 65536

    match &expr.kind {
        // Integer literals
        ExprKind::Lit(lit) => match lit.node {
            rustc_ast::LitKind::Int(val, _) => val.get() < SMALL_LITERAL_THRESHOLD,
            // Floats don't overflow in the same way (they become infinity/NaN)
            rustc_ast::LitKind::Float(_, _) => true,
            _ => false,
        },
        // Unary negation of small literals (e.g., -1, -100)
        ExprKind::Unary(rustc_hir::UnOp::Neg, inner) => is_small_literal(inner),
        // Type casts of small literals (e.g., 1 as u64)
        ExprKind::Cast(inner, _) => is_small_literal(inner),
        _ => false,
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
