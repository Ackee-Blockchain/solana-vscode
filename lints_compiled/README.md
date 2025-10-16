# Pre-compiled Lint Libraries

This directory contains pre-compiled dylint libraries for different platforms.

## Structure

- `macos-arm64/` - macOS Apple Silicon
- `macos-x64/` - macOS Intel
- `linux-x64/` - Linux x86_64
- `linux-arm64/` - Linux ARM64

## Building

To build lints for your current platform:

```bash
cd ../lints
./build_all_lints.sh
```

## Distribution

These pre-compiled libraries are bundled with the VSCode extension so that users don't need to compile them locally.

## Library Naming

Libraries follow the pattern:
```
lib{lint_name}@{toolchain}.{ext}
```

Example:
- `libsimple_test_lint@nightly-2025-08-07-aarch64-apple-darwin.dylib`
- `libsimple_test_lint@nightly-2025-08-07-x86_64-unknown-linux-gnu.so`

