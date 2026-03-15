# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

VS Code extension for Solana/Anchor development by Ackee Blockchain Security. Provides real-time static analysis security detectors and Trident fuzzing coverage visualization. Published as `AckeeBlockchain.solana` on the VS Code Marketplace.

## Repository Structure

Two main components:

- **`extension/`** — VS Code extension (TypeScript, Webpack-bundled)
- **`language-server/`** — Rust LSP server (Tower-LSP + Tokio)
- **`extension/detectors/`** — Dylint-based custom lint detectors (Rust cdylib crates)

## Build Commands

### Extension (TypeScript)
```bash
cd extension
npm ci                    # Install dependencies
npm run build             # Webpack production build → dist/extension.js
npm run watch             # Webpack watch mode
npm run lint              # ESLint
npm run test              # Run extension tests (requires pretest: webpack test config)
```

### Language Server (Rust)
```bash
cd language-server
cargo build               # Debug build
cargo build --release     # Release build
cargo test --verbose      # Run all detector tests
cargo fmt -- --check      # Format check
```

The `language-server/build-language-server.sh` script builds and copies the binary to `extension/bin/language-server`. The `.vscode/tasks.json` has pre-configured build tasks for development.

### Dylint Detectors
```bash
./build_all_lints.sh      # Compiles all detectors, outputs to lints_compiled/$PLATFORM/
```
Requires Rust nightly `nightly-2025-09-18` with `llvm-tools-preview` and `rustc-dev` components.

## Architecture

### Extension Side (`extension/src/`)

Entry point: `extension.ts` — activates on `Anchor.toml` or `trident-tests` presence.

Three feature managers initialized in `extensionFeatureManagers.ts`:
- **DetectorsManager** — launches the bundled Rust LSP binary, communicates via `vscode-languageclient`. Listens for `solana/scanComplete` and `solana/detectorStatus` notifications.
- **CoverageManager** — loads Trident coverage reports (`cargo llvm-cov` JSON), applies line decorations via VS Code Test API.
- **StatusBarManager** — monitors Rust nightly toolchain and dylint tool installation status.

### Language Server (`language-server/src/`)

- **`backend.rs`** — core LSP implementation (`LanguageServer` trait), orchestrates scanning and diagnostics
- **`core/detectors/detector.rs`** — `Detector` trait that all built-in detectors implement (`id`, `name`, `analyze` → `Vec<Diagnostic>`)
- **`core/registry/detector_registry.rs`** — registry pattern; stores detectors, runs enabled ones on file content
- **`core/file_scanner/scanner.rs`** — recursively finds `.rs` files, excludes `target/`, `node_modules/`, `trident-tests/`, test files
- **`core/dylint/`** — manages compilation, loading, and caching of external dylint lints
- **`dylint_runner/`** — shells out to `cargo-dylint`, parses TOML diagnostic output

### Built-in Detectors (in language server)
Each detector lives in `core/detectors/` and has a corresponding test in `language-server/tests/`:
- `immutable_account_mutated_detector` — immutable account mutation
- `instruction_attribute_invalid` / `instruction_attribute_unused`
- `manual_lamports_zeroing` — unsafe lamports patterns
- `missing_check_comment` — missing security check comments
- `missing_initspace_detector` — missing InitSpace macro
- `sysvar_account_detector` — improper sysvar access

### External Dylint Detectors (`extension/detectors/`)
Compiled as `cdylib` crates, loaded dynamically at runtime:
- `missing_signer/` — missing signer verification
- `unchecked_math/` — unchecked arithmetic
- `immutable_account_mutated/` — (in progress)

## CI

GitHub Actions (`.github/workflows/ci.yml`) runs on PRs to main:
- **language-server job**: `cargo fmt --check`, `cargo build`, `cargo test`
- **extension job**: `npm ci`, `npm run lint`, `npm run build`, `xvfb-run -a npm test`

## Development Workflow

Use VS Code launch configurations in `.vscode/launch.json` for debugging. The "Build Extension and Language Server" task in `.vscode/tasks.json` does a full build. For iterative development, use `npm run watch` for the extension and rebuild the language server as needed.

The extension bundles the language server binary at `extension/bin/language-server`. The `server.path` setting can override this for local development.
