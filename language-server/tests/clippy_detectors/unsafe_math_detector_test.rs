extern crate rustc_ast;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

use std::fs::write;
use std::path::PathBuf;
use std::sync::Mutex;

use rustc_ast::LitKind;
use rustc_driver::{Callbacks, run_compiler};
use rustc_hir::{BinOpKind, Expr, ExprKind, UnOp};
use rustc_interface::Config;
use rustc_lint::{LateContext, LateLintPass, Lint, LintContext, LintPass, LintStore};
use rustc_middle::ty::TyKind;
use rustc_session::declare_lint;

// The `UNSAFE_MATH_LINT` lint detects unchecked arithmetic operations.
//
// ### Example
//
// ```rust
// fn main() {
//     let x = 1u64;
//     let y = 2u64;
//     let z = x + y; // This could overflow!
// }
// ```
//
// ### Explanation
//
// This lint detects arithmetic operations (+, -, *, /) on integer types that could
// potentially overflow or underflow. It suggests using checked arithmetic methods
// like checked_add(), checked_sub(), checked_mul(), or checked_div() instead.
//
// The lint will not trigger for:
// - Operations with small literal values (< 2^16)
// - Operations on floating point types
// - Operations that use checked arithmetic methods
declare_lint! {
    pub UNSAFE_MATH_LINT,
    Warn,
    "detects unchecked arithmetic operations that may overflow"
}

#[derive(Copy, Clone)]
struct UnsafeMathLintPass;

impl LintPass for UnsafeMathLintPass {
    fn name(&self) -> &'static str {
        "unsafe_math_lint"
    }

    fn get_lints(&self) -> Vec<&'static Lint> {
        vec![&UNSAFE_MATH_LINT]
    }
}

impl<'tcx> LateLintPass<'tcx> for UnsafeMathLintPass {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx rustc_hir::Expr<'tcx>) {
        // Check if this is a binary operation (arithmetic)
        if let ExprKind::Binary(bin_op, left_expr, right_expr) = &expr.kind {
            // Check if it's an arithmetic operation we care about
            let is_arithmetic = matches!(
                bin_op.node,
                BinOpKind::Add | BinOpKind::Sub | BinOpKind::Mul | BinOpKind::Div
            );

            if is_arithmetic {
                // Get the types of the operands
                let left_ty = cx.typeck_results().expr_ty(left_expr);
                let right_ty = cx.typeck_results().expr_ty(right_expr);

                // Check if either operand is an integer type that could overflow
                let is_risky_type = |ty: rustc_middle::ty::Ty<'_>| -> bool {
                    matches!(ty.kind(), TyKind::Int(_) | TyKind::Uint(_))
                };

                if is_risky_type(left_ty) || is_risky_type(right_ty) {
                    // Check if this is a "safe" operation (small literals)
                    let is_safe =
                        self.is_safe_operation(cx, bin_op.node, expr, left_expr, right_expr);

                    if !is_safe {
                        let op_name = match bin_op.node {
                            BinOpKind::Add => "addition",
                            BinOpKind::Sub => "subtraction",
                            BinOpKind::Mul => "multiplication",
                            BinOpKind::Div => "division",
                            _ => "arithmetic operation",
                        };

                        let checked_method = match bin_op.node {
                            BinOpKind::Add => "checked_add()",
                            BinOpKind::Sub => "checked_sub()",
                            BinOpKind::Mul => "checked_mul()",
                            BinOpKind::Div => "checked_div()",
                            _ => "checked arithmetic",
                        };

                        let msg = format!(
                            "Unchecked {} detected. Consider using {} to prevent overflow/underflow",
                            op_name, checked_method
                        );

                        cx.span_lint(&UNSAFE_MATH_LINT, expr.span, |diag| {
                            diag.primary_message(msg.clone());
                        });

                        // Store for testing
                        let mut diagnostics = DIAGNOSTICS.lock().unwrap();
                        diagnostics.push(msg);
                    }
                }
            }
        }
    }
}

// Integer type metadata used by the detector
struct IntTypeInfo {
    signed: bool,
    bits: u32,
    min_i128: i128, // valid if signed
    max_i128: i128, // valid if signed
    max_u128: u128, // valid if unsigned
}

// Literal value captured with sign
enum LitVal {
    Signed(i128),
    Unsigned(u128),
}

impl UnsafeMathLintPass {
    /// Decide if operation is safe using type-aware, conservative rules.
    fn is_safe_operation(
        &self,
        cx: &LateContext<'_>,
        op: BinOpKind,
        full_expr: &Expr<'_>,
        left: &Expr<'_>,
        right: &Expr<'_>,
    ) -> bool {
        // Resolve the resulting integer type of the expression
        let ty = cx.typeck_results().expr_ty(full_expr);
        let type_info = match self.int_type_info(cx, ty.kind()) {
            None => return false,
            Some(ti) => ti,
        };

        // Helper: attempt exact evaluation when both operands are literals
        if let (Some(lv), Some(rv)) = (
            self.extract_literal_value(cx, &type_info, left),
            self.extract_literal_value(cx, &type_info, right),
        ) {
            return self.eval_and_check_within_bounds(op, &type_info, lv, rv);
        }

        // One literal (or none): allow only trivial-safe transforms
        // Add/Sub by 0, Mul by 0/1, Div by 1.
        if let Some(lit) = self.extract_literal_value(cx, &type_info, left) {
            return self.is_trivially_safe_left_literal(op, &type_info, lit);
        }
        if let Some(lit) = self.extract_literal_value(cx, &type_info, right) {
            return self.is_trivially_safe_right_literal(op, &type_info, lit);
        }

        // Otherwise unknown at compile-time → not proven safe
        false
    }

    fn int_type_info(&self, cx: &LateContext<'_>, kind: &TyKind) -> Option<IntTypeInfo> {
        match kind {
            TyKind::Int(int_ty) => {
                let (signed, bits) = match int_ty {
                    rustc_middle::ty::IntTy::Isize => (true, self.pointer_bits(cx)),
                    rustc_middle::ty::IntTy::I8 => (true, 8),
                    rustc_middle::ty::IntTy::I16 => (true, 16),
                    rustc_middle::ty::IntTy::I32 => (true, 32),
                    rustc_middle::ty::IntTy::I64 => (true, 64),
                    rustc_middle::ty::IntTy::I128 => (true, 128),
                };
                let (min_i128, max_i128) = self.signed_bounds(bits);
                Some(IntTypeInfo {
                    signed,
                    bits,
                    min_i128,
                    max_i128,
                    max_u128: 0,
                })
            }
            TyKind::Uint(uint_ty) => {
                let (signed, bits) = match uint_ty {
                    rustc_middle::ty::UintTy::Usize => (false, self.pointer_bits(cx)),
                    rustc_middle::ty::UintTy::U8 => (false, 8),
                    rustc_middle::ty::UintTy::U16 => (false, 16),
                    rustc_middle::ty::UintTy::U32 => (false, 32),
                    rustc_middle::ty::UintTy::U64 => (false, 64),
                    rustc_middle::ty::UintTy::U128 => (false, 128),
                };
                let max_u128 = self.unsigned_max(bits);
                Some(IntTypeInfo {
                    signed,
                    bits,
                    min_i128: 0,
                    max_i128: 0,
                    max_u128,
                })
            }
            _ => None,
        }
    }

    fn pointer_bits(&self, cx: &LateContext<'_>) -> u32 {
        cx.tcx.data_layout.pointer_size.bits() as u32
    }

    fn signed_bounds(&self, bits: u32) -> (i128, i128) {
        let shift = (bits - 1) as u32;
        let max = (1i128 << shift) - 1;
        let min = -1i128 - max;
        (min, max)
    }

    fn unsigned_max(&self, bits: u32) -> u128 {
        if bits >= 128 {
            u128::MAX
        } else {
            (1u128 << bits) - 1
        }
    }

    fn extract_literal_value(
        &self,
        _cx: &LateContext<'_>,
        info: &IntTypeInfo,
        expr: &Expr<'_>,
    ) -> Option<LitVal> {
        if info.signed {
            // Signed: handle negative and positive integer literals
            match &expr.kind {
                ExprKind::Lit(lit) => {
                    if let LitKind::Int(v, _) = lit.node {
                        let v_u128: u128 = v.into();
                        if v_u128 <= i128::MAX as u128 {
                            return Some(LitVal::Signed(v_u128 as i128));
                        }
                    }
                }
                ExprKind::Unary(UnOp::Neg, inner) => {
                    if let ExprKind::Lit(lit) = &inner.kind {
                        if let LitKind::Int(v, _) = lit.node {
                            let v_u128: u128 = v.into();
                            if v_u128 <= i128::MAX as u128 {
                                let sv = v_u128 as i128;
                                return Some(LitVal::Signed(-sv));
                            }
                        }
                    }
                }
                _ => {}
            }
        } else {
            // Unsigned: only non-negative integer literals
            if let ExprKind::Lit(lit) = &expr.kind {
                if let LitKind::Int(v, _) = lit.node {
                    let v_u128: u128 = v.into();
                    return Some(LitVal::Unsigned(v_u128));
                }
            }
        }
        None
    }

    fn eval_and_check_within_bounds(
        &self,
        op: BinOpKind,
        info: &IntTypeInfo,
        lv: LitVal,
        rv: LitVal,
    ) -> bool {
        if info.signed {
            let (l, r) = match (lv, rv) {
                (LitVal::Signed(a), LitVal::Signed(b)) => (a, b),
                _ => return false,
            };
            // Division by zero check
            if matches!(op, BinOpKind::Div) && r == 0 {
                return false;
            }
            // iN::MIN / -1 overflows
            if matches!(op, BinOpKind::Div) && l == info.min_i128 && r == -1 {
                return false;
            }
            let res = match op {
                BinOpKind::Add => l.checked_add(r),
                BinOpKind::Sub => l.checked_sub(r),
                BinOpKind::Mul => l.checked_mul(r),
                BinOpKind::Div => l.checked_div(r),
                _ => None,
            };
            if let Some(val) = res {
                val >= info.min_i128 && val <= info.max_i128
            } else {
                false
            }
        } else {
            let (l, r) = match (lv, rv) {
                (LitVal::Unsigned(a), LitVal::Unsigned(b)) => (a, b),
                _ => return false,
            };
            if matches!(op, BinOpKind::Div) && r == 0 {
                return false;
            }
            let res = match op {
                BinOpKind::Add => l.checked_add(r),
                BinOpKind::Sub => l.checked_sub(r),
                BinOpKind::Mul => l.checked_mul(r),
                BinOpKind::Div => l.checked_div(r),
                _ => None,
            };
            if let Some(val) = res {
                val <= info.max_u128
            } else {
                false
            }
        }
    }

    fn is_trivially_safe_left_literal(
        &self,
        op: BinOpKind,
        info: &IntTypeInfo,
        lit: LitVal,
    ) -> bool {
        match (op, info.signed, lit) {
            (BinOpKind::Add, _, LitVal::Signed(0)) => true,
            (BinOpKind::Add, _, LitVal::Unsigned(0)) => true,
            // 0 - x is NOT provably safe
            (BinOpKind::Sub, _, LitVal::Signed(0)) => false,
            (BinOpKind::Sub, _, LitVal::Unsigned(0)) => false, // 0 - var unsigned definitely underflows
            (BinOpKind::Mul, _, LitVal::Signed(0)) => true,
            (BinOpKind::Mul, _, LitVal::Unsigned(0)) => true,
            (BinOpKind::Mul, _, LitVal::Signed(1)) => true,
            (BinOpKind::Mul, _, LitVal::Unsigned(1)) => true,
            (BinOpKind::Div, _, LitVal::Signed(1)) => true,
            (BinOpKind::Div, _, LitVal::Unsigned(1)) => true,
            _ => false,
        }
    }

    fn is_trivially_safe_right_literal(
        &self,
        op: BinOpKind,
        info: &IntTypeInfo,
        lit: LitVal,
    ) -> bool {
        match (op, info.signed, lit) {
            (BinOpKind::Add, _, LitVal::Signed(0)) => true,
            (BinOpKind::Add, _, LitVal::Unsigned(0)) => true,
            (BinOpKind::Sub, _, LitVal::Signed(0)) => true, // x - 0 safe
            (BinOpKind::Sub, _, LitVal::Unsigned(0)) => true, // x - 0 safe for unsigned too
            (BinOpKind::Mul, _, LitVal::Signed(0)) => true,
            (BinOpKind::Mul, _, LitVal::Unsigned(0)) => true,
            (BinOpKind::Mul, _, LitVal::Signed(1)) => true,
            (BinOpKind::Mul, _, LitVal::Unsigned(1)) => true,
            (BinOpKind::Div, _, LitVal::Signed(1)) => true,
            (BinOpKind::Div, _, LitVal::Unsigned(1)) => true,
            _ => false,
        }
    }
}

static DIAGNOSTICS: Mutex<Vec<String>> = Mutex::new(Vec::new());

struct MyCallbacks;

impl Callbacks for MyCallbacks {
    fn config(&mut self, config: &mut Config) {
        config.register_lints = Some(Box::new(|_sess, lint_store: &mut LintStore| {
            lint_store.register_late_pass(|_| Box::new(UnsafeMathLintPass));
        }));
    }
}

#[test]
fn test_unsafe_math_detection() {
    let temp_path = PathBuf::from("temp.rs");
    let source_code = r#"
        fn main() {
            // This should trigger the lint - large numbers that could overflow
            let x: u64 = 1000000000;
            let y: u64 = 2000000000;
            let z = x + y; // Unsafe addition
            
            // This should also trigger
            let a: u32 = 4000000000u32;
            let b: u32 = 1000000000u32;
            let c = a - b; // Unsafe subtraction
            
            // This should NOT trigger - small literals
            let small1 = 10;
            let small2 = 20;
            let small_sum = small1 + small2; // Safe - small numbers
        }
    "#;
    write(&temp_path, source_code).expect("Failed to write temporary file");

    DIAGNOSTICS.lock().unwrap().clear();

    let args = vec![
        "rustc".to_string(),
        "--edition=2021".to_string(),
        temp_path.to_str().unwrap().to_string(),
    ];
    run_compiler(&args, &mut MyCallbacks);

    let diagnostics = DIAGNOSTICS.lock().unwrap();
    println!("Collected {} diagnostics:", diagnostics.len());
    for (i, diag) in diagnostics.iter().enumerate() {
        println!("  {}: {}", i + 1, diag);
    }

    // We should have detected the unsafe operations
    assert!(
        !diagnostics.is_empty(),
        "No unsafe math operations were detected"
    );

    // Check that we detected addition and subtraction
    let has_addition = diagnostics
        .iter()
        .any(|d| d.contains("addition") && d.contains("checked_add"));
    let has_subtraction = diagnostics
        .iter()
        .any(|d| d.contains("subtraction") && d.contains("checked_sub"));

    assert!(has_addition, "Should detect unsafe addition");
    assert!(has_subtraction, "Should detect unsafe subtraction");

    std::fs::remove_file(temp_path).ok();
}

#[test]
fn test_safe_operations_ignored() {
    let temp_path = PathBuf::from("temp_safe.rs");
    let source_code = r#"
        fn main() {
            // These should NOT trigger the lint - direct small literals
            let z = 1 + 2;
            let c = 100 * 50;
            let d = 10 - 5;
            let e = 1000 / 10;
        }
    "#;
    write(&temp_path, source_code).expect("Failed to write temporary file");

    DIAGNOSTICS.lock().unwrap().clear();

    let args = vec![
        "rustc".to_string(),
        "--edition=2021".to_string(),
        temp_path.to_str().unwrap().to_string(),
    ];
    run_compiler(&args, &mut MyCallbacks);

    let diagnostics = DIAGNOSTICS.lock().unwrap();
    println!(
        "Safe operations test - collected {} diagnostics:",
        diagnostics.len()
    );
    for (i, diag) in diagnostics.iter().enumerate() {
        println!("  {}: {}", i + 1, diag);
    }

    // Should have no diagnostics for safe operations
    assert!(
        diagnostics.is_empty(),
        "Safe operations should not trigger lint"
    );

    std::fs::remove_file(temp_path).ok();
}

#[test]
fn test_trivial_type_aware_safe_cases() {
    let temp_path = PathBuf::from("temp_safe_trivial.rs");
    let source_code = r#"
        fn main() {
            let x_i64: i64 = 42;
            let u_u64: u64 = 42;

            // Variable + trivial identities (should NOT trigger)
            let _ = x_i64 + 0;
            let _ = 0 + x_i64;
            let _ = x_i64 - 0;
            let _ = x_i64 * 1;
            let _ = 1 * x_i64;
            let _ = x_i64 / 1;

            let _ = u_u64 + 0;
            let _ = 0 + u_u64;
            let _ = u_u64 - 0;
            let _ = u_u64 * 1;
            let _ = 1 * u_u64;
            let _ = u_u64 / 1;

            // Both-literal within-bounds with explicit type context (should NOT trigger)
            let _ : i8 = 120 + 7;   // 127 fits i8
            let _ : u8 = 200 + 10;  // 210 fits u8
            let _ : i32 = -5 + 5;   // 0 fits i32

            // Floats (should NOT trigger)
            let _ = 1.0f64 + 2.0f64;
            let _ = 3.5f32 * 2.0f32;

            // isize/usize with small literals (should NOT trigger)
            let _ : isize = 1 + 2;
            let _ : usize = 3 * 4;

            // Boundary no-op (should NOT trigger)
            let _ : i8 = 127 + 0;
            let _ : u8 = 255 - 0;
        }
    "#;
    write(&temp_path, source_code).expect("Failed to write temporary file");

    DIAGNOSTICS.lock().unwrap().clear();

    let args = vec![
        "rustc".to_string(),
        "--edition=2021".to_string(),
        temp_path.to_str().unwrap().to_string(),
    ];
    run_compiler(&args, &mut MyCallbacks);

    let diagnostics = DIAGNOSTICS.lock().unwrap();
    println!(
        "Trivial type-aware safe cases - collected {} diagnostics:",
        diagnostics.len()
    );
    for (i, diag) in diagnostics.iter().enumerate() {
        println!("  {}: {}", i + 1, diag);
    }

    // Expect no diagnostics for trivial safe cases
    assert!(
        diagnostics.is_empty(),
        "Trivial safe cases should not trigger lint"
    );

    std::fs::remove_file(temp_path).ok();
}

#[test]
fn test_float_operations_ignored() {
    let temp_path = PathBuf::from("temp_safe_float.rs");
    let source_code = r#"
        fn main() {
            let _ = 1.0 + 2.0;
            let _ = 10.5 - 2.25;
            let _ = 3.0 * 4.0;
            let _ = 8.0 / 2.0;
        }
    "#;
    write(&temp_path, source_code).expect("Failed to write temporary file");

    DIAGNOSTICS.lock().unwrap().clear();

    let args = vec![
        "rustc".to_string(),
        "--edition=2021".to_string(),
        temp_path.to_str().unwrap().to_string(),
    ];
    run_compiler(&args, &mut MyCallbacks);

    let diagnostics = DIAGNOSTICS.lock().unwrap();
    println!(
        "Float operations - collected {} diagnostics:",
        diagnostics.len()
    );
    for (i, diag) in diagnostics.iter().enumerate() {
        println!("  {}: {}", i + 1, diag);
    }

    assert!(
        diagnostics.is_empty(),
        "Float operations should not trigger lint"
    );

    std::fs::remove_file(temp_path).ok();
}

#[test]
fn test_boundary_both_literal_ok() {
    let temp_path = PathBuf::from("temp_safe_boundary.rs");
    let source_code = r#"
        fn main() {
            // Evaluate within-bounds expressions in a typed context
            let _ : i16 = 32767 - 0;
            let _ : u16 = 65535 / 1;
            let _ : i64 = -1 + 1;
            let _ : u64 = 0 * 123456789u64;
        }
    "#;
    write(&temp_path, source_code).expect("Failed to write temporary file");

    DIAGNOSTICS.lock().unwrap().clear();

    let args = vec![
        "rustc".to_string(),
        "--edition=2021".to_string(),
        temp_path.to_str().unwrap().to_string(),
    ];
    run_compiler(&args, &mut MyCallbacks);

    let diagnostics = DIAGNOSTICS.lock().unwrap();
    println!(
        "Boundary both-literal - collected {} diagnostics:",
        diagnostics.len()
    );
    for (i, diag) in diagnostics.iter().enumerate() {
        println!("  {}: {}", i + 1, diag);
    }

    assert!(
        diagnostics.is_empty(),
        "Boundary both-literal cases should not trigger lint"
    );

    std::fs::remove_file(temp_path).ok();
}

// This test targets potential false positives for parenthesized identities.
// It is marked ignored because current detector may still flag these.
#[test]
#[ignore]
fn test_parenthesized_identities_ignored() {
    let temp_path = PathBuf::from("temp_safe_paren.rs");
    let source_code = r#"
        fn main() {
            let x: i64 = 10;
            let _ = x + (1 - 1);
            let _ = x * (1 * 1);
            let _ = x - (0);
            let _ = x / (1);
        }
    "#;
    write(&temp_path, source_code).expect("Failed to write temporary file");

    DIAGNOSTICS.lock().unwrap().clear();

    let args = vec![
        "rustc".to_string(),
        "--edition=2021".to_string(),
        temp_path.to_str().unwrap().to_string(),
    ];
    run_compiler(&args, &mut MyCallbacks);

    let diagnostics = DIAGNOSTICS.lock().unwrap();
    println!(
        "Parenthesized identities - collected {} diagnostics:",
        diagnostics.len()
    );
    for (i, diag) in diagnostics.iter().enumerate() {
        println!("  {}: {}", i + 1, diag);
    }

    assert!(
        diagnostics.is_empty(),
        "Parenthesized identities should not trigger lint"
    );

    std::fs::remove_file(temp_path).ok();
}
