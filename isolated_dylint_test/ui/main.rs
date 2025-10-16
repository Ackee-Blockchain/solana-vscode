fn main() {
    // This should trigger the lint - addition operations
    let x = 5 + 10;

    let a = 1;
    let b = 2;
    let c = a + b;

    // This should NOT trigger - subtraction
    let d = 10 - 5;

    // This should NOT trigger - multiplication
    let e = 3 * 4;
}
