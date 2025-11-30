# Testing Dylint Detector Support

## Prerequisites

1. **Nightly Rust installed**:
   ```bash
   rustup toolchain install nightly
   ```

2. **Dylint dependencies**:
   ```bash
   cargo install cargo-dylint dylint-link
   cargo +nightly dylint --list  # This installs dylint-driver
   ```

## Creating a Test Dylint Detector

1. **Create a test detector crate** in your workspace:

```bash
cd /path/to/your/workspace
cargo new --lib my_detector
cd my_detector
```

2. **Update `Cargo.toml`**:

```toml
[package]
name = "my_detector"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["dylib"]

[dependencies]
dylint = "2.1.0"
```

3. **Create `src/lib.rs`**:

```rust
use dylint_linting::Lint;

#[dylint_lib::export_lint]
pub fn my_detector(_cx: &dylint_linting::EarlyContext<'_>) {
    // Your detector logic here
}
```

## Testing Steps

### 1. Build the Language Server

```bash
cd language-server
cargo build
```

### 2. Run the Language Server

The language server will automatically:
- Scan for dylint detectors in the workspace
- Compile them with nightly Rust (first time)
- Cache the compiled libraries
- Load them dynamically

### 3. Check Logs

Look for these log messages:
- `"Scanning for dylint detector crates in: ..."`
- `"Found dylint detector: ..."`
- `"Compiling dylint detector ... with nightly ..."`
- `"Cached compiled detector for future reuse"`
- `"Successfully loaded detector from ..."`

### 4. Verify Cache

Check the cache directory:
```bash
ls ~/.cache/solana-vscode/dylint-detectors/
```

You should see cached `.dylib` (macOS) or `.so` (Linux) files.

### 5. Test Cache Reuse

1. Restart the language server
2. Check logs - you should see:
   - `"Reusing cached detector: ... (nightly ...)"`
   - No compilation messages

### 6. Test Nightly Version Change

1. Install a different nightly version
2. Restart the language server
3. It should detect the version change and recompile

## Manual Testing Commands

You can also test the components separately:

### Test Scanner
```rust
// In a test or example
let mut scanner = DylintDetectorScanner::new();
scanner.set_workspace_root(PathBuf::from("/path/to/workspace"));
let detectors = scanner.scan_detectors();
println!("Found detectors: {:?}", detectors);
```

### Test Compiler
```rust
let compiler = DylintDetectorCompiler::new();
let nightly = DylintDetectorCompiler::get_nightly_version()?;
let compiled = compiler.compile_detector(&detector, &nightly).await?;
```

### Test Cache
```rust
let cache = DylintDetectorCache::new()?;
let cached = cache.get_cached_library(&detector, &nightly);
```

## Expected Behavior

✅ **First run**: Compiles detectors, caches them, loads them
✅ **Subsequent runs**: Reuses cached builds (fast)
✅ **Nightly change**: Automatically recompiles with new nightly
✅ **Workspace change**: Rescans and loads new detectors

## Troubleshooting

- **"Failed to get nightly Rust version"**: Install nightly with `rustup toolchain install nightly`
- **"Failed to compile detector"**: Check that the detector crate has proper `Cargo.toml` with `crate-type = ["dylib"]`
- **"No dylint detectors found"**: Make sure detector crates have `dylint` dependency and proper lib configuration
