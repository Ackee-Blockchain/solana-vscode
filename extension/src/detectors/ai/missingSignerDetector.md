# Missing Signer Detector

## Overview

This detector identifies Anchor program accounts structs that don't have any signer fields, which could allow unauthorized access to sensitive operations.

## What to Look For

1. Look for structs that derive from `Accounts` in Anchor programs.
2. Check if any field in the struct has the `Signer` type.
3. If no signer field is found, flag this as a security issue.

## Technical Details

In Solana programs using the Anchor framework:

- Account structs are typically marked with `#[derive(Accounts)]`
- A signer field would be of type `Signer<'info>` or similar
- Without a signer check, anyone could call the instruction, potentially leading to unauthorized access

## Example of Vulnerable Code

```rust
#[derive(Accounts)]
pub struct TransferFunds<'info> {
    #[account(mut)]
    pub from: Account<'info, TokenAccount>,
    #[account(mut)]
    pub to: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}
```

## Example of Secure Code

```rust
#[derive(Accounts)]
pub struct TransferFunds<'info> {
    #[account(mut)]
    pub from: Account<'info, TokenAccount>,
    #[account(mut)]
    pub to: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    // Added signer field for authorization
    pub authority: Signer<'info>,
}
```

## Detection Rules

1. Parse Rust code to find structs with `#[derive(Accounts)]`
2. For each such struct, check all fields to find any with type `Signer<'info>`
3. If no signer field is found, report a warning on the struct definition line

## Output Format

For each detection, provide:

- File path
- Line number of the struct definition
- Warning message: "Accounts struct has no signer. Consider adding a Signer<'info> field to ensure proper authorization."
- Severity: "warning"
- Detector ID: "AI_MISSING_SIGNER"
