# missing_signer

### What it does

Detects Anchor program instructions that have no signer accounts, which could allow unauthorized access.

### Why is this bad?

In Solana programs, missing signer checks can lead to serious security vulnerabilities:

- Anyone can call the instruction without authorization
- Unauthorized users can modify program state
- Attackers can drain funds or manipulate data
- No way to verify who initiated the transaction

### Example

```rust
// Bad - No signer field
#[derive(Accounts)]
pub struct Transfer<'info> {
    #[account(mut)]
    pub from: Account<'info, TokenAccount>,
    #[account(mut)]
    pub to: Account<'info, TokenAccount>,
}

pub fn transfer(ctx: Context<Transfer>, amount: u64) -> Result<()> {
    // No signer - anyone can transfer from any account!
    Ok(())
}
```

Use instead:

```rust
// Good - Has signer field
#[derive(Accounts)]
pub struct Transfer<'info> {
    pub authority: Signer<'info>,
    #[account(mut)]
    pub from: Account<'info, TokenAccount>,
    #[account(mut)]
    pub to: Account<'info, TokenAccount>,
}

pub fn transfer(ctx: Context<Transfer>, amount: u64) -> Result<()> {
    // authority.key() can be used to verify ownership
    Ok(())
}
```
