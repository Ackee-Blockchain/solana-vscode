# Change Log

All notable changes to the "solana" extension will be documented in this file.

## [0.0.1]

- Prepare the extension for development

## [0.0.2]

- Added extension icon

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
