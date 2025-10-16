# Solana VSCode Extension - Custom Lints

This directory contains isolated dylint lints for the Solana VSCode extension.

## Structure

Each lint is its own independent crate with:
- `src/lib.rs` - The lint implementation
- `Cargo.toml` - Dependencies and metadata
- `rust-toolchain` - Pinned nightly toolchain version
- `ui/` - UI tests for the lint

## Available Lints

### addition_detector
A test lint that detects addition operations (`+`). Used to verify the dylint integration works correctly.

## Creating a New Lint

1. **Scaffold a new lint** using `cargo dylint`:
```bash
cd lints
cargo dylint new my_detector
cd my_detector
```

This creates a complete lint project with:
- `src/lib.rs` - Lint implementation template
- `Cargo.toml` - Dependencies pre-configured
- `rust-toolchain` - Pinned nightly version
- `ui/` - UI test directory

2. **Verify it builds**:
```bash
cargo build
cargo dylint list --path .
```

You should see your lint listed.

3. **Implement your lint** in `src/lib.rs`:

All you need to do is implement the `LateLintPass` trait. Example:

```rust
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_lint;

use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};

dylint_linting::declare_late_lint! {
    pub MY_DETECTOR,
    Warn,
    "detects something suspicious"
}

impl<'tcx> LateLintPass<'tcx> for MyDetector {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        // Your lint logic here
        if let ExprKind::SomePattern = expr.kind {
            clippy_utils::diagnostics::span_lint(
                cx,
                MY_DETECTOR,
                expr.span,
                "your warning message",
            );
        }
    }
}
```

4. **Add UI tests** in `ui/` directory (test code that should trigger your lint)

5. **Build all lints**:
```bash
cd ..
./build_all_lints.sh
```

Your new lint is automatically discovered and loaded by the extension!

## Building Lints

Run the build script from the `lints` directory:
```bash
./build_all_lints.sh
```

This will compile all lints and copy the resulting `.dylib`/`.so` files to `../lints_compiled/<platform>/`.

## Testing Lints

Each lint can be tested individually:
```bash
cd simple_test_lint
cargo test
```

Or run against a test project:
```bash
cd simple_test_lint
cargo dylint simple_test_lint -- --manifest-path=/path/to/test/project/Cargo.toml
```

