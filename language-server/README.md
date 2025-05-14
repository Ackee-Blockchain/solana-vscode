# Solana Language Server

A Language Server Protocol (LSP) implementation for Solana smart contracts, providing IDE features like code completion, diagnostics, and more.

## Overview

This language server is built using Rust and implements the Language Server Protocol to provide enhanced IDE support for Solana smart contract development. It integrates with the Solana VSCode extension to deliver a seamless development experience.

## Features

- Syntax validation for Solana smart contracts
- Code completion and suggestions
- Real-time diagnostics
- Semantic analysis of Rust code

## Technical Details

- Built with Rust 2024 edition
- Uses `tower-lsp` for LSP implementation
- Leverages `syn` for Rust code parsing and analysis
- Asynchronous runtime powered by `tokio`

## Development

### Prerequisites

- Rust toolchain (latest stable version)
- Cargo package manager

### Building

```bash
cargo build
```

### Running

```bash
cargo run
```

### Debugging

Use the provided `debug-run.zsh` script to run the language server in debug mode:

```bash
./debug-run.zsh
```

## Dependencies

- tower-lsp = "0.20.0"
- tokio = { version = "1.0", features = ["full"] }
- log = "0.4"
- env_logger = "0.11.8"
- syn = { version = "2.0.101", features = ["full", "extra-traits", "visit", "parsing"] }

## Project Structure

- `src/main.rs` - Entry point and server initialization
- `src/server.rs` - LSP server implementation
- `src/backend.rs` - Core language server functionality

## License

This project is part of the Solana VSCode extension. See the main project for license details.
