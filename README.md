# Solana VSCode Extension

A VSCode extension for Solana development that provides security scanning and code coverage visualization for Solana programs.

## Features

### Security Scanning

The extension provides security scanning for Solana programs with the following detectors:

- **Immutable Account Mutated**: Detects when an immutable account is being mutated
- **Instruction Attribute Invalid**: Detects invalid instruction attributes
- **Instruction Attribute Unused**: Detects unused instruction attributes
- **Manual Lamports Zeroing**: Detects manual lamports zeroing which can lead to security issues
- **Missing Check Comment**: Detects missing check comments in critical code sections
- **Missing InitSpace**: Detects missing initialization space in account creation
- **Missing Signer**: Detects missing signer verification
- **Sysvar Account**: Detects improper sysvar account access
- **Unsafe Math**: Detects unsafe mathematical operations that could lead to overflows

### Code Coverage

The extension provides code coverage visualization for Trident tests, helping you identify untested code in your Solana programs.

### Commands

- `solana: Scan Workspace for Security Issues` (Ctrl+Alt+S / Cmd+Alt+S): Scans your workspace for security issues
- `solana: Reload Security Detectors` (Ctrl+Alt+R / Cmd+Alt+R): Reloads security detectors
- `solana: Show Code Coverage`: Shows code coverage for Trident tests
- `solana: Close Code Coverage`: Closes code coverage visualization
- `solana: Show Security Scan Output`: Shows security scan output

## Running the Extension Locally

1. Clone this repository

```bash
git clone git@github.com:Ackee-Blockchain/solana-vscode.git
cd solana-vscode
```

2. Install dependencies

```bash
cd extension && npm i
```

3. Open the project in Cursor

```bash
cursor .
```

4. Press F5 to start the Extension Development Host

This will:

- Open a new VS Code Extension Development Host window
- Load your extension
- Enable you to test all features
- Allow you to set breakpoints and debug the extension

Note: Make sure you have Node.js and npm installed on your system before starting.

## Development

This extension consists of two main components:

1. The VSCode extension (TypeScript)
2. The Language Server (Rust)

### Prerequisites

- Node.js (v20 or later)
- Rust and Cargo (latest stable)
- VSCode

### Project Structure

```
solana-vscode/
├── extension/          # TypeScript extension code
│   ├── src/           # Source code
│   ├── bin/           # Language server binary
│   └── package.json   # Extension manifest
└── language-server/   # Rust language server code
    └── src/          # Server source code
```

### Development Workflow

The extension supports several development workflows:

1. **Standard Development**:

   ```bash
   # One-time build
   F5 or "Run Extension" configuration
   ```

2. **Watch Mode**:

   ```bash
   # Command Palette > Tasks: Run Task > Watch Extension and Build Language Server
   # or
   cd extension && npm run watch
   ```

3. **Language Server Development**:
   ```bash
   # Build language server only
   cd extension && ./build-language-server.sh
   ```

### Available Tasks

- `Build Extension and Language Server`: One-time build
- `Watch Extension and Build Language Server`: Build server once and watch extension
- `Build Extension and Language Server (Debug)`: Debug builds

### Testing

```bash
# Extension tests
cd extension && npm test

# Language server tests
cd language-server && cargo test
```

### Packaging

The extension is packaged per platform with its corresponding language server binary. GitHub Actions workflow handles this automatically for releases.

Local packaging:

```bash
cd extension
./build-language-server.sh
npm run build
vsce package
```
