# unsafe_type_cast

### What it does

Detects potentially unsafe integer type casts that could silently truncate values or produce unexpected results due to sign changes in Solana programs.

### Why is this bad?

Narrowing casts (e.g., `u64` to `u32`) silently discard upper bits, which can lead to token amount manipulation, incorrect balance calculations, or logic bugs. Signed-to-unsigned casts (e.g., `i64` to `u64`) can turn negative values into large positive values.

### Known problems

- `isize`/`usize` are treated as 64-bit (correct for Solana BPF target, but not portable).
- Same-width unsigned-to-signed casts (e.g., `u32` to `i32`) are not flagged, as these are common and usually intentional.

### Example

```rust
let amount_u32 = amount_u64 as u32; // silently truncates
let value = signed_val as u64;       // negative becomes large positive
```

Use instead:

```rust
let amount_u32: u32 = amount_u64.try_into().map_err(|_| ErrorCode::Overflow)?;
let value: u64 = signed_val.try_into().map_err(|_| ErrorCode::InvalidValue)?;
```
