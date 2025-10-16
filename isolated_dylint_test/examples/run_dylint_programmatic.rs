use std::env;
/// Run dylint programmatically using cargo check with RUSTC_WORKSPACE_WRAPPER
/// This is the closest to "true" programmatic execution without subprocess to cargo
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn main() -> anyhow::Result<()> {
    println!("Running dylint programmatically...\n");

    // Build the lint first to ensure .dylib exists
    println!("Building lint library...");
    let build_status = Command::new("cargo")
        .args(&["build", "--release"])
        .current_dir(".")
        .status()?;

    if !build_status.success() {
        anyhow::bail!("Failed to build lint library");
    }

    // Find the built .dylib file with toolchain suffix
    let toolchain = "nightly-2025-08-07-aarch64-apple-darwin"; // From rust-toolchain file
    let lint_lib = std::fs::read_dir("target/release")?
        .filter_map(|e| e.ok())
        .find(|e| {
            let name = e.file_name();
            let name_str = name.to_string_lossy();
            name_str.starts_with("libdylint_test")
                && name_str.contains(&format!("@{}", toolchain))
                && (name_str.ends_with(".dylib") || name_str.ends_with(".so"))
        })
        .ok_or_else(|| anyhow::anyhow!("Could not find lint library with toolchain suffix"))?;

    let lint_lib_path = lint_lib.path().canonicalize()?;
    println!("Found lint library: {}", lint_lib_path.display());

    // Get dylint-driver path
    let home = env::var("HOME")?;
    let dylint_driver = PathBuf::from(format!(
        "{}/.dylint_drivers/{}/dylint-driver",
        home, toolchain
    ));

    if !dylint_driver.exists() {
        anyhow::bail!(
            "dylint-driver not found at {:?}. Run 'cargo dylint list' first to install it.",
            dylint_driver
        );
    }

    println!("Using dylint-driver: {}", dylint_driver.display());
    println!("\nCleaning previous build...");

    // Clean first to force recompilation
    Command::new("cargo")
        .arg("clean")
        .current_dir("simple_counter")
        .status()?;

    println!("Running cargo check with dylint driver...\n");

    // Run cargo check with RUSTC_WORKSPACE_WRAPPER set to dylint-driver
    let output = Command::new("cargo")
        .args(&["check", "--message-format=json"])
        .current_dir("simple_counter")
        .env("RUSTC_WORKSPACE_WRAPPER", dylint_driver)
        .env("DYLINT_LIBS", format!("[\"{}\"]", lint_lib_path.display()))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    // Convert output to string
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Filter for our lint diagnostics only
    let mut diagnostics = Vec::new();

    for line in stdout.lines() {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
            if json.get("reason").and_then(|r| r.as_str()) == Some("compiler-message") {
                if let Some(message) = json.get("message") {
                    if let Some(code) = message
                        .get("code")
                        .and_then(|c| c.get("code"))
                        .and_then(|c| c.as_str())
                    {
                        if code == "no_addition" {
                            diagnostics.push(message.clone());
                        }
                    }
                }
            }
        }
    }

    println!("Found {} diagnostics from our lint", diagnostics.len());

    // Write to JSON file
    let output_file = "programmatic_diagnostics.json";
    let json_output = serde_json::to_string_pretty(&diagnostics)?;

    let mut file = File::create(output_file)?;
    file.write_all(json_output.as_bytes())?;

    println!("✅ Diagnostics written to: {}", output_file);
    println!("\nSummary:");

    for (i, diag) in diagnostics.iter().enumerate() {
        let msg = diag
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown");
        let file = diag
            .get("spans")
            .and_then(|s| s.as_array())
            .and_then(|arr| arr.first())
            .and_then(|span| span.get("file_name"))
            .and_then(|f| f.as_str())
            .unwrap_or("unknown");
        let line = diag
            .get("spans")
            .and_then(|s| s.as_array())
            .and_then(|arr| arr.first())
            .and_then(|span| span.get("line_start"))
            .and_then(|l| l.as_u64())
            .unwrap_or(0);

        println!("  {}. {} at {}:{}", i + 1, msg, file, line);
    }

    Ok(())
}
