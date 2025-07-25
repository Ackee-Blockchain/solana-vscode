use language_server::core::{
    ClippyUncheckedArithmeticDetector, DetectorRegistryBuilder, UnsafeMathDetector,
};

#[tokio::main]
async fn main() {
    env_logger::init();

    // Create a hybrid detector registry with both syn and clippy detectors
    let mut registry = DetectorRegistryBuilder::new()
        .with_syn_detector(UnsafeMathDetector::default()) // Syn-based (fast)
        .with_clippy_detector(ClippyUncheckedArithmeticDetector::new()) // Clippy-style (comprehensive)
        .build();

    // Sample Rust code with arithmetic operations
    let sample_code = r#"
fn main() {
    let a = 100;
    let b = 200;
    let c: f32 = 100.0;
    let d: f32 = 100.0;
    // This should be detected by both detectors
    let result1 = a + b;
    
    // This should be safe (explicit types)
    let result2 = c + d;
    
    // This should be detected as unsafe
    let result3 = a * b * 1000;
    
    // This should be safe (checked arithmetic)
    let result4 = a.checked_add(b);
}
"#;

    println!("=== Hybrid Detector Demo ===\n");
    println!("Sample code:");
    println!("{}", sample_code);
    println!("\n=== Analysis Results ===\n");

    // Run immediate syn-based analysis
    println!("1. Immediate Syn-based Analysis (fast):");
    let immediate_diagnostics = registry.analyze_immediate(sample_code, None);

    for diagnostic in &immediate_diagnostics {
        let code_str = match diagnostic.code.as_ref().unwrap() {
            tower_lsp::lsp_types::NumberOrString::String(s) => s.as_str(),
            tower_lsp::lsp_types::NumberOrString::Number(n) => &n.to_string(),
        };
        println!(
            "  - {} (Line {}): {}",
            code_str,
            diagnostic.range.start.line + 1,
            diagnostic.message
        );
    }

    if immediate_diagnostics.is_empty() {
        println!("  No immediate issues detected");
    }

    // Run comprehensive clippy-style analysis
    println!("\n2. Comprehensive Clippy-style Analysis (slower, type-aware):");

    // Create a temporary file for clippy analysis
    let temp_file = std::path::PathBuf::from("/tmp/demo.rs");
    tokio::fs::write(&temp_file, sample_code).await.unwrap();

    let comprehensive_diagnostics = registry
        .analyze_comprehensive(&temp_file, sample_code)
        .await;

    for diagnostic in &comprehensive_diagnostics {
        let code_str = match diagnostic.code.as_ref().unwrap() {
            tower_lsp::lsp_types::NumberOrString::String(s) => s.as_str(),
            tower_lsp::lsp_types::NumberOrString::Number(n) => &n.to_string(),
        };
        println!(
            "  - {} (Line {}): {}",
            code_str,
            diagnostic.range.start.line + 1,
            diagnostic.message
        );
    }

    if comprehensive_diagnostics.is_empty() {
        println!("  No comprehensive issues detected");
    }

    // Clean up
    let _ = tokio::fs::remove_file(&temp_file).await;

    println!("\n=== Summary ===");
    println!("Syn detectors: Fast, immediate feedback, syntax-based");
    println!("Clippy detectors: Slower, type-aware, comprehensive analysis");
    println!("Hybrid approach: Best of both worlds!");
}
