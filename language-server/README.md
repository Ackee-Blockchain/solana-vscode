# Solana Security Analysis Language Server

A Language Server Protocol (LSP) implementation for Solana smart contract security analysis, providing real-time security diagnostics and workspace scanning.

## Overview

This language server provides automated security analysis for Solana smart contract development. It integrates with the Solana VSCode extension to deliver real-time security feedback for Solana/Anchor projects.

## Features

- **Real-time Security Diagnostics**: Instant feedback on security issues
- **Workspace Scanning**: Analyze entire projects for vulnerabilities
- **Multiple Security Detectors**: Built-in analyzers for common security issues
- **Anchor Program Support**: Specialized analysis for Anchor framework

### Security Detectors

1. **Unsafe Math Detector**: Identifies arithmetic overflow/underflow vulnerabilities
2. **Missing Signer Detector**: Detects missing signature verification
3. **Manual Lamports Zeroing Detector**: Identifies improper account balance manipulation
4. **Sysvar Account Detector**: Validates proper sysvar account usage

## Development

### Prerequisites

- Rust nightly toolchain (`nightly-2025-09-18`)
  - Install with: `rustup toolchain install nightly-2025-09-18`
  - Required components: `llvm-tools-preview`, `rustc-dev`
- Cargo package manager
- dylint-driver for running security detectors
  - Install with: `cargo install cargo-dylint dylint-link`
  - Initialize with: `cargo +nightly-2025-09-18 dylint --list`

### Building

```bash
cargo build
```

### Running

```bash
cargo run
```

### Testing

```bash
cargo test
```

### Debugging

```bash
# Debug level (default)
./debug-run.zsh

# With specific log level
./debug-run.zsh info
```

## Dependencies

- `tower-lsp = "0.20.0"` - LSP implementation
- `tokio = { version = "1.45.1", features = ["full"] }` - Async runtime
- `syn = { version = "2.0.101", features = ["full", "extra-traits", "visit", "parsing"] }` - Rust parsing
- `serde = { version = "1.0", features = ["derive"] }` - Serialization
- `log = "0.4"` - Logging
- `env_logger = "0.11.8"` - Logger configuration

## Project Structure

```
src/
├── main.rs              # Entry point
├── server.rs            # LSP server setup
├── backend.rs           # Core implementation
└── core/
    ├── detectors/       # Security detectors
    ├── file_scanner/    # Workspace scanning
    ├── registry/        # Detector management
    └── utilities/       # Helper functions

tests/                   # Test suite
```

## Usage

### Integration with VSCode

- Automatic project scanning when opened
- Real-time security issue highlighting
- Manual workspace scan via `workspace.scan` command
- Issues displayed in Problems panel

### Supported Files

- Rust files (`.rs`) - Primary analysis
- `Anchor.toml` - Anchor configuration
- `Cargo.toml` - Rust project configuration

## License

This project is part of the Solana VSCode extension. See the main project for license details.
