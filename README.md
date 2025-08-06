# Solana

A Visual Studio Code extension for Solana development that provides security scanning and code coverage visualization for Solana programs.

![Solana Extension](extension/assets/icon.png)

## Security Scanning

Enhance your Solana development workflow with built-in security scanning. The extension automatically detects common security issues in your Solana programs:

- **Immutable Account Mutated**: Identifies when code attempts to modify an account marked as immutable
- **Instruction Attribute Invalid**: Detects invalid instruction attributes that could cause runtime errors
- **Instruction Attribute Unused**: Finds unused instruction attributes that might indicate logic errors
- **Manual Lamports Zeroing**: Detects unsafe manual lamports zeroing patterns
- **Missing Check Comment**: Identifies critical code sections lacking security check comments
- **Missing InitSpace**: Catches account creation without proper space initialization
- **Missing Signer**: Alerts when code fails to verify required signers
- **Sysvar Account**: Detects improper sysvar account access methods
- **Unsafe Math**: Identifies mathematical operations that could lead to overflows

## Code Coverage

Visualize your test coverage directly in the editor:

- See which lines are covered by your Trident tests
- View execution counts for each line
- Quickly identify untested code paths
- Customize the appearance of coverage indicators

## Quick Access Commands

- `solana: Scan Workspace for Security Issues` (Ctrl+Alt+S / Cmd+Alt+S)
- `solana: Reload Security Detectors` (Ctrl+Alt+R / Cmd+Alt+R)
- `solana: Show Code Coverage`
- `solana: Close Code Coverage`
- `solana: Show Security Scan Output`

## Requirements

- Visual Studio Code 1.96.0 or newer
- Rust and Cargo (latest stable) for Solana program security scanning
- Trident tests in your workspace for code coverage features

## Getting Started

1. Install the extension from the Visual Studio Code Marketplace
2. Open a Solana project in VS Code
3. Use the command palette (Ctrl+Shift+P / Cmd+Shift+P) to run:
   - `solana: Scan Workspace for Security Issues` to scan for security vulnerabilities
   - `solana: Show Code Coverage` to visualize code coverage from Trident tests

## Extension Settings

- `server.path`: Path to the Solana language server binary (leave empty to use bundled version)
- `tridentCoverage.showExecutionCount`: Show execution count numbers next to covered statements
- `tridentCoverage.executionCountColor`: Color of the execution count display
- `tridentCoverage.coverageServerPort`: Port for the coverage server
