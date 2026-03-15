# immutable_account_mutated

### What it does

Detects attempts to mutate Anchor accounts that are not marked as mutable with `#[account(mut)]`.

### Why is this bad?

In Solana programs, attempting to mutate an account that is not declared as mutable will cause a runtime error. This detector catches these issues at development time:

- The Solana runtime will reject transactions that modify accounts not marked as writable
- Missing `#[account(mut)]` means changes to the account will be silently discarded or cause a transaction failure
- This is a common source of bugs when working with Anchor programs

### Example

```rust
// Bad - Account is not marked as mutable but is being mutated
#[derive(Accounts)]
pub struct UpdateVault<'info> {
    pub vault: Account<'info, Vault>,
}

pub fn update_vault(ctx: Context<UpdateVault>, amount: u64) -> Result<()> {
    ctx.accounts.vault.amount = amount; // Mutation of immutable account!
    Ok(())
}
```

Use instead:

```rust
// Good - Account is marked as mutable
#[derive(Accounts)]
pub struct UpdateVault<'info> {
    #[account(mut)]
    pub vault: Account<'info, Vault>,
}

pub fn update_vault(ctx: Context<UpdateVault>, amount: u64) -> Result<()> {
    ctx.accounts.vault.amount = amount; // OK - account is mutable
    Ok(())
}
```
