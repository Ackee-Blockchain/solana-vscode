# Change Log

All notable changes to the "solana" extension will be documented in this file.

## [1.0.3] - Prerelease

### Improved

- **Rust Toolchain Management**: Extension now requires and checks for specific nightly toolchain (nightly-2025-09-18)

  - One-click installation of required nightly toolchain from status bar
  - Automatic detection and warning if toolchain is missing

- **dylint-driver Integration**: Added automatic checking and installation support for dylint-driver

  - Status bar shows warnings if dylint-driver is not installed
  - One-click installation command
  - Comprehensive setup flow for both nightly toolchain and dylint-driver

- **Detector Status Feedback**: Real-time status updates during security scanning

  - Status bar shows spinning icon when detectors are initializing, building, or running
  - Clear visual feedback on scanning progress
  - Automatic status updates on completion

- **Optimized File Scanning**:

  - Only scans Rust (.rs) files in workspace (excludes external dependencies)
  - Skips test directories and test files for faster scanning
  - Skips build artifacts, node_modules, and other non-source directories
  - Removed unnecessary Anchor.toml and Cargo.toml scanning

- **Detector Initialization**: Detectors are now built and run automatically on project open

  - Detectors also run on every file save
  - Background processing doesn't block editor

- **Better Diagnostics Display**:
  - Detector names are now shown in UPPERCASE for better visibility
  - Clearer distinction between detector name and message

### Fixed

- Improved directory filtering to exclude all external dependencies
- Better test file detection (supports standard Rust patterns)
- Removed unused code and streamlined data structures

## [0.1.2]

### Security Detectors

- Added security detectors for Solana programs:
  - Immutable Account Mutated: Detects when an immutable account is being mutated
  - Instruction Attribute Invalid: Detects invalid instruction attributes
  - Instruction Attribute Unused: Detects unused instruction attributes
  - Manual Lamports Zeroing: Detects manual lamports zeroing which can lead to security issues
  - Missing Check Comment: Detects missing check comments in critical code sections
  - Missing InitSpace: Detects missing initialization space in account creation
  - Missing Signer: Detects missing signer verification
  - Sysvar Account: Detects improper sysvar account access
  - Unsafe Math: Detects unsafe mathematical operations that could lead to overflows

### Features

- Added security scanning for Solana programs
- Added code coverage visualization for Trident tests
- Added workspace scanning command with keyboard shortcut (Ctrl+Alt+S / Cmd+Alt+S)
- Added detector reload command with keyboard shortcut (Ctrl+Alt+R / Cmd+Alt+R)

## [0.0.2]

- Added extension icon

## [0.0.1]

- Prepare the extension for development
