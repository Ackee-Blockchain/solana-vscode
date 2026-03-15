fn main() {
    let amount_u64: u64 = 1_000_000;
    let amount_u128: u128 = 1_000_000;
    let signed_val: i64 = -1;
    let small: u16 = 100;

    // Should trigger: narrowing casts
    let _a = amount_u64 as u32;
    let _b = amount_u64 as u16;
    let _c = amount_u64 as u8;
    let _d = amount_u128 as u64;

    // Should trigger: signed-to-unsigned casts
    let _e = signed_val as u64;
    let _f = signed_val as u128;

    // Should NOT trigger: widening unsigned casts
    let _g = small as u32;
    let _h = small as u64;

    // Should NOT trigger: small literal casts
    let _i = 1u64 as u32;
    let _j = 255 as u8;

    // Should NOT trigger: same-width unsigned-to-signed
    let _k = amount_u64 as i64;
}
