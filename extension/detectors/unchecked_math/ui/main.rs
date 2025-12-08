fn main() {
    let x = 1 + 2; // Should trigger warning
    let y = x + 3; // Should trigger warning
    let z = x - y; // Should NOT trigger (subtraction)
    let w = x * y; // Should NOT trigger (multiplication)
}
