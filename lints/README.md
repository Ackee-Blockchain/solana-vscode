# Dylint Lints for Solana Security

This directory contains custom [Dylint](https://github.com/trailofbits/dylint) lints for detecting security vulnerabilities in Solana programs.

## Available Lints

### `unchecked_math`

Detects unchecked arithmetic operations that could lead to overflow/underflow vulnerabilities.

**What it detects:**

- Unchecked addition (`+`)
- Unchecked subtraction (`-`)
- Unchecked multiplication (`*`)
- Unchecked division (`/`)
- Compound assignments (`+=`, `-=`, `*=`, `/=`)

**Example:**

```rust
// ❌ BAD - Will be flagged
let total = amount1 + amount2;
let balance -= withdrawal;

// ✅ GOOD - Won't be flagged
let total = amount1.checked_add(amount2).ok_or(ErrorCode::Overflow)?;
let balance = balance.checked_sub(withdrawal).ok_or(ErrorCode::Underflow)?;
```

**Features:**

- Checks all integer types (u8-u128, i8-i128, usize, isize)
- Smart literal detection (skips small constants like `i + 1`)
- Provides actionable suggestions

## Building Lints

### Quick Build (All Lints)

```bash
# From project root
./build_all_lints.sh
```

### Manual Build (Single Lint)

```bash
cd lints/unchecked_math
cargo build
# Or for release:
cargo build --release
```

### Build Output

Compiled lints are placed in:

```
lints_compiled/
├── macos-arm64/
│   └── libunchecked_math@*.dylib
├── macos-x64/
├── linux-x64/
└── linux-arm64/
```

## Testing Lints

### Run Tests

```bash
cd lints/unchecked_math
cargo test
```

### Test on a Project

```bash
cd /path/to/your/solana/project

DYLINT_LIBS='["/path/to/libunchecked_math@*.dylib"]' \
RUSTC_WORKSPACE_WRAPPER=$HOME/.dylint_drivers/nightly-2025-09-18-aarch64-apple-darwin/dylint-driver \
cargo +nightly-2025-09-18 check
```

## Creating a New Lint

### 1. Scaffold the Lint

```bash
cd lints/
cargo dylint new my_new_lint
```

### 2. Copy Toolchain File

```bash
cp unchecked_math/rust-toolchain my_new_lint/
```

### 3. Implement the Lint

Edit `my_new_lint/src/lib.rs`:

```rust
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;

use rustc_hir::Expr;
use rustc_lint::{LateContext, LateLintPass};

dylint_linting::declare_late_lint! {
    /// ### What it does
    /// Describe what your lint detects
    ///
    /// ### Why is this bad?
    /// Explain the security implications
    ///
    /// ### Example
    /// Show good and bad examples
    pub MY_NEW_LINT,
    Warn,
    "short description"
}

impl<'tcx> LateLintPass<'tcx> for MyNewLint {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        // Your lint logic here
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
```

### 4. Add Test Cases

Create `my_new_lint/ui/main.rs`:

```rust
// Test cases that should trigger your lint
fn main() {
    // Bad code here
}
```

Create `my_new_lint/ui/main.stderr`:

```
warning: your lint message
 --> $DIR/main.rs:3:5
  |
3 |     // Bad code here
  |     ^^^^^^^^^^^^^^^^
  |
  = help: your suggestion
```

### 5. Build and Test

```bash
cd my_new_lint
cargo test
cargo build
```

### 6. Integrate with Extension

```bash
cd ../..
./build_all_lints.sh
cp lints_compiled/macos-arm64/*.dylib extension/lints_compiled/macos-arm64/
```

## Lint Development Tips

### Using `clippy_utils`

For common patterns, use `clippy_utils`:

```rust
use clippy_utils::diagnostics::span_lint_and_help;

// In your lint:
span_lint_and_help(
    cx,
    MY_LINT,
    expr.span,
    "your message",
    None,
    "your suggestion",
);
```

### Checking Types

```rust
use rustc_middle::ty::TyKind;

let ty = cx.typeck_results().expr_ty(expr);
if matches!(ty.kind(), TyKind::Uint(_)) {
    // It's an unsigned integer
}
```

### Skipping Macro Expansions

```rust
if expr.span.from_expansion() {
    return; // Skip macros
}
```

### Accessing HIR

```rust
use rustc_hir::{Expr, ExprKind};

match expr.kind {
    ExprKind::Binary(op, left, right) => {
        // Handle binary operations
    }
    _ => {}
}
```

## Toolchain Management

All lints use the same Rust toolchain specified in `rust-toolchain`:

```toml
[toolchain]
channel = "nightly-2025-09-18"
components = ["llvm-tools-preview", "rustc-dev"]
```

**Important:** All lints MUST use the same toolchain version!

## Dependencies

### Common Dependencies

```toml
[dependencies]
clippy_utils = { git = "https://github.com/rust-lang/rust-clippy" }
dylint_linting = "4.1.0"
```

### Dev Dependencies

```toml
[dev-dependencies]
dylint_testing = "4.1.0"
```

## Troubleshooting

### "can't find crate for `rustc_*`"

Install rustc-dev:

```bash
rustup component add rustc-dev --toolchain nightly-2025-09-18
```

### "clippy_utils version mismatch"

Use the master branch:

```toml
clippy_utils = { git = "https://github.com/rust-lang/rust-clippy" }
```

### "dylint-driver not found"

Install it:

```bash
cargo install cargo-dylint dylint-link
cargo +nightly-2025-09-18 dylint --list
```

## Resources

- [Dylint Documentation](https://github.com/trailofbits/dylint)
- [Clippy Lint Development](https://doc.rust-lang.org/nightly/clippy/development/adding_lints.html)
- [Rustc Dev Guide](https://rustc-dev-guide.rust-lang.org/)
- [HIR Documentation](https://doc.rust-lang.org/nightly/nightly-rustc/rustc_hir/)

## Contributing

When adding a new lint:

1. ✅ Use the same toolchain as existing lints
2. ✅ Add comprehensive tests
3. ✅ Document what it detects and why
4. ✅ Provide good/bad examples
5. ✅ Test on real Solana programs
6. ✅ Update this README

## License

Same as the parent project.
