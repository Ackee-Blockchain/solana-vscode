# immutable_account_mutated

### What it does

Detects attempts to mutate Anchor accounts that are not marked as mutable with `#[account(mut)]`.

### Why is this bad?

In Solana/Anchor programs, attempting to mutate an immutable account will cause a runtime error:

- The transaction will fail with "Account is not writable"
- Wastes compute units and transaction fees
- Can cause unexpected program failures
- Violates the Anchor framework's safety guarantees

### Example

```rust
// Bad - Account not marked as mutable
#[derive(Accounts)]
pub struct UpdateData<'info> {
    pub data_account: Account<'info, DataAccount>,  // Missing #[account(mut)]
}

pub fn update(ctx: Context<UpdateData>) -> Result<()> {
    ctx.accounts.data_account.value = 42;  // Error: trying to mutate!
    Ok(())
}
```

Use instead:

```rust
// Good - Account marked as mutable
#[derive(Accounts)]
pub struct UpdateData<'info> {
    #[account(mut)]  // Now marked as mutable
    pub data_account: Account<'info, DataAccount>,
}

pub fn update(ctx: Context<UpdateData>) -> Result<()> {
    ctx.accounts.data_account.value = 42;  // OK!
    Ok(())
}
```
