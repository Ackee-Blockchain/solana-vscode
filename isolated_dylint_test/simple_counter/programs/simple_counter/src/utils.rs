/// Helper function that performs addition
pub fn calculate_double(value: u64) -> u64 {
    // This should trigger the lint - addition in utility module
    value + value
}

/// Another helper with addition
pub fn add_numbers(a: u64, b: u64) -> u64 {
    // This should also trigger the lint
    a + b
}

/// Calculate sum of array
pub fn sum_array(numbers: &[u64]) -> u64 {
    let mut total = 0;
    for num in numbers {
        // This should trigger the lint multiple times
        total = total + num;
    }
    total
}
