// Test file to verify dylint unchecked_math detector works
// This file should trigger warnings from the dylint lint

fn main() {
    // Should trigger: unchecked addition
    let a: u64 = 100;
    let b: u64 = 200;
    let total = a + b;

    // Should trigger: unchecked subtraction
    let balance: u64 = 1000;
    let withdrawal: u64 = 500;
    let remaining = balance - withdrawal;

    // Should trigger: unchecked multiplication
    let price: u32 = 100;
    let quantity: u32 = 50;
    let cost = price * quantity;

    // Should trigger: unchecked division
    let numerator: i64 = 1000;
    let denominator: i64 = 10;
    let result = numerator / denominator;

    // Should trigger: compound assignment
    let mut counter: usize = 0;
    counter += 1;

    // Should NOT trigger: small literals
    let index = 0 + 1;
    let offset = index * 2;

    // Should NOT trigger: checked operations
    let safe_total = a.checked_add(b).unwrap();
    let safe_remaining = balance.checked_sub(withdrawal).unwrap();

    println!("Total: {}, Remaining: {}, Cost: {}, Result: {}, Counter: {}",
             total, remaining, cost, result, counter);
    println!("Safe: {}, {}", safe_total, safe_remaining);
}
