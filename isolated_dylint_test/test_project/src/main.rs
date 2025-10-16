fn main() {
    // This should trigger the lint - addition operation
    let x = 5 + 10;
    println!("Result: {}", x);

    let a = 1;
    let b = 2;
    let c = a + b; // Another addition

    println!("Sum: {}", c);
}
