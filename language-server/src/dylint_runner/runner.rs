use super::diagnostics::DylintDiagnostic;
use super::parser::parse_json_output;
use anyhow::{Context, Result};
use log::{debug, info, warn};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug)]
pub struct DylintRunner {
    /// Path to pre-compiled lint libraries (e.g., lints_compiled/macos-arm64/)
    lint_libs_dir: PathBuf,

    /// List of lint library files to load (pre-compiled + workspace detectors)
    lint_libs: Arc<std::sync::Mutex<Vec<PathBuf>>>,

    /// Cache of last run results per workspace
    cache: Arc<Mutex<std::collections::HashMap<PathBuf, Vec<DylintDiagnostic>>>>,
}

impl DylintRunner {
    /// Add workspace detector libraries to the runner
    pub fn add_workspace_detectors(&self, detector_libs: Vec<PathBuf>) {
        let mut libs = self.lint_libs.lock().unwrap();
        info!("Adding {} workspace detector(s) to dylint runner", detector_libs.len());
        libs.extend(detector_libs);
        info!("Dylint runner now has {} total lint(s)", libs.len());
    }

    /// Initialize the runner with pre-compiled lints (if available)
    /// Can start empty and have detectors added later via add_workspace_detectors
    pub fn new(extension_path: &Path) -> Result<Self> {
        // 1. Detect platform
        let platform = Self::detect_platform()?;

        // 2. Find pre-compiled lints directory
        let lint_libs_dir = extension_path.join("lints_compiled").join(platform);

        // 3. Discover all .dylib/.so files in the directory (if it exists)
        let lint_libs = if lint_libs_dir.exists() {
            match Self::discover_lint_libs(&lint_libs_dir) {
                Ok(libs) => {
                    if libs.is_empty() {
                        info!(
                            "No pre-compiled lints found in {}. Runner will start empty.",
                            lint_libs_dir.display()
                        );
                        Vec::new()
                    } else {
                        info!(
                            "Dylint runner initialized with {} pre-compiled lints from {}",
                            libs.len(),
                            lint_libs_dir.display()
                        );
                        libs
                    }
                }
                Err(e) => {
                    warn!("Failed to discover pre-compiled lints: {}. Starting with empty runner.", e);
                    Vec::new()
                }
            }
        } else {
            info!(
                "Pre-compiled lints directory not found: {}. Runner will start empty and can have detectors added later.",
                lint_libs_dir.display()
            );
            Vec::new()
        };

        Ok(Self {
            lint_libs_dir,
            lint_libs: Arc::new(std::sync::Mutex::new(lint_libs)),
            cache: Arc::new(Mutex::new(std::collections::HashMap::new())),
        })
    }

    /// Run lints on a workspace
    pub async fn run_lints(&self, workspace_path: &Path) -> Result<Vec<DylintDiagnostic>> {
        // Clone the lint libs list while holding the lock, then release it
        let lint_libs: Vec<PathBuf> = {
            let libs = self.lint_libs.lock().unwrap();
            if libs.is_empty() {
                return Ok(Vec::new());
            }
            libs.clone()
        };

        debug!(
            "Running dylint lints on workspace: {}",
            workspace_path.display()
        );

        // Check if workspace has Cargo.toml
        let cargo_toml = workspace_path.join("Cargo.toml");
        if !cargo_toml.exists() {
            debug!("No Cargo.toml found in workspace, skipping dylint");
            return Ok(Vec::new());
        }

        // Build PATH with cargo bin directories
        let current_path = std::env::var("PATH").unwrap_or_default();
        let home = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;

        let cargo_bin = home.join(".cargo").join("bin");
        let new_path = format!(
            "{}:/usr/local/bin:/usr/bin:{current_path}",
            cargo_bin.display()
        );

        // Detect toolchain from lint's rust-toolchain file
        let toolchain = Self::detect_lint_toolchain(&self.lint_libs_dir)?;
        debug!("Using toolchain: {}", toolchain);

        // Get dylint-driver path
        let dylint_driver = Self::find_dylint_driver(&home, &toolchain)?;
        debug!("Using dylint-driver: {}", dylint_driver.display());

        // Build DYLINT_LIBS JSON array with absolute paths
        let dylint_libs_json = serde_json::to_string(
            &lint_libs
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
        )?;

        debug!("DYLINT_LIBS: {}", dylint_libs_json);

        // Run cargo check with dylint
        let output = tokio::process::Command::new("cargo")
            .arg(format!("+{}", toolchain))
            .args(&["check", "--workspace", "--message-format=json"])
            .current_dir(workspace_path)
            .env("PATH", new_path)
            .env("RUSTC_WORKSPACE_WRAPPER", &dylint_driver)
            .env("DYLINT_LIBS", dylint_libs_json)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("Failed to spawn cargo check")?;

        // Extract lint names from loaded libraries
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Debug: log cargo output
        debug!("[Dylint] Cargo stdout length: {} bytes", stdout.len());
        debug!("[Dylint] Cargo stderr length: {} bytes", stderr.len());
        if !stderr.is_empty() {
            debug!("[Dylint] Cargo stderr: {}", stderr.lines().take(10).collect::<Vec<_>>().join("\n"));
        }

        let lint_codes: Vec<String> = lint_libs
            .iter()
            .filter_map(|path| {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .and_then(|s| s.split('@').next())
                    .map(|s| s.strip_prefix("lib").unwrap_or(s).to_string())
            })
            .collect();

        // Parse JSON output
        debug!("[Dylint] Parsing output for lint codes: {:?}", lint_codes);
        let diagnostics = parse_json_output(&stdout, &lint_codes)?;
        debug!("[Dylint] Parsed {} diagnostic(s)", diagnostics.len());

        // Update cache
        {
            let mut cache = self.cache.lock().await;
            cache.insert(workspace_path.to_path_buf(), diagnostics.clone());
        }

        info!("Dylint found {} issues", diagnostics.len());
        Ok(diagnostics)
    }

    /// Detect current platform
    fn detect_platform() -> Result<&'static str> {
        match (std::env::consts::OS, std::env::consts::ARCH) {
            ("macos", "aarch64") => Ok("macos-arm64"),
            ("macos", "x86_64") => Ok("macos-x64"),
            ("linux", "x86_64") => Ok("linux-x64"),
            ("linux", "aarch64") => Ok("linux-arm64"),
            (os, arch) => Err(anyhow::anyhow!(
                "Unsupported platform: {}-{}. \
                Supported: macos-arm64, macos-x64, linux-x64, linux-arm64",
                os,
                arch
            )),
        }
    }

    /// Detect the Rust toolchain from the lint library filename
    fn detect_lint_toolchain(lints_dir: &Path) -> Result<String> {
        use crate::core::dylint::constants::REQUIRED_NIGHTLY_VERSION;

        // Simply return the required nightly version - all detectors use this version
        info!("Using extension's required nightly version: {}", REQUIRED_NIGHTLY_VERSION);
        return Ok(REQUIRED_NIGHTLY_VERSION.to_string());

        // Old detection code kept as fallback (commented out)
        /*
        // Extract toolchain from lint library filename
        // Format: libunchecked_math@nightly-2025-09-18-aarch64-apple-darwin.dylib
        if let Ok(entries) = std::fs::read_dir(lints_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let file_name = entry.file_name();
                let file_name_str = file_name.to_string_lossy();

                // Look for @ in filename
                if let Some(at_pos) = file_name_str.find('@') {
                    let after_at = &file_name_str[at_pos + 1..];

                    // Extract toolchain: "nightly-2025-09-18-aarch64..." -> "nightly-2025-09-18"
                    // Date format is YYYY-MM-DD, so we need 4 parts: nightly-YYYY-MM-DD
                    let parts: Vec<&str> = after_at.split('-').collect();
                    if parts.len() >= 5 && parts[0] == "nightly" {
                        // parts[0] = "nightly", parts[1] = "2025", parts[2] = "09", parts[3] = "18", parts[4] = "aarch64"
                        let toolchain = format!("{}-{}-{}-{}", parts[0], parts[1], parts[2], parts[3]);
                        info!("Detected toolchain from lint filename: {}", toolchain);
                        return Ok(toolchain);
                    }
                }
            }
        }
        */

        // Fallback: Look for rust-toolchain file
        let lints_parent = lints_dir
            .parent()
            .and_then(|p| p.parent())
            .ok_or_else(|| anyhow::anyhow!("Could not find lints directory"))?
            .join("lints");

        if let Ok(entries) = std::fs::read_dir(&lints_parent) {
            for entry in entries.filter_map(|e| e.ok()) {
                let rust_toolchain = entry.path().join("rust-toolchain");
                if rust_toolchain.exists()
                    && let Ok(content) = std::fs::read_to_string(&rust_toolchain)
                {
                    for line in content.lines() {
                        if line.trim().starts_with("channel")
                            && let Some(channel) = line.split('"').nth(1)
                        {
                            info!("Detected toolchain from rust-toolchain file: {}", channel);
                            return Ok(channel.to_string());
                        }
                    }
                }
            }
        }

        // This code is now unreachable since we return early above
        // But kept for safety
        warn!("Could not detect toolchain, falling back to required nightly");
        Ok(REQUIRED_NIGHTLY_VERSION.to_string())
    }

    /// Find dylint-driver executable
    fn find_dylint_driver(home: &Path, toolchain: &str) -> Result<PathBuf> {
        let arch = std::env::consts::ARCH;
        let os = match std::env::consts::OS {
            "macos" => "apple-darwin",
            "linux" => "unknown-linux-gnu",
            _ => "unknown",
        };

        let toolchain_target = format!("{}-{}-{}", toolchain, arch, os);
        let dylint_driver = home
            .join(".dylint_drivers")
            .join(&toolchain_target)
            .join("dylint-driver");

        if !dylint_driver.exists() {
            anyhow::bail!(
                "dylint-driver not found at {:?}.\n\
                Install it by running: cargo install cargo-dylint dylint-link\n\
                Then run: cargo +{} dylint --list",
                dylint_driver,
                toolchain
            );
        }

        Ok(dylint_driver)
    }

    /// Discover all lint libraries in the directory
    fn discover_lint_libs(dir: &Path) -> Result<Vec<PathBuf>> {
        let mut libs = Vec::new();

        for entry in std::fs::read_dir(dir)
            .context(format!("Failed to read directory: {}", dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

                // Check if it's a lint library (.dylib, .so, .dll)
                if name.starts_with("lib")
                    && (name.ends_with(".dylib") || name.ends_with(".so") || name.ends_with(".dll"))
                {
                    libs.push(path);
                }
            }
        }

        Ok(libs)
    }

    /// Get list of loaded lint names
    pub fn loaded_lints(&self) -> Vec<String> {
        let lint_libs = self.lint_libs.lock().unwrap();
        lint_libs
            .iter()
            .filter_map(|p| {
                p.file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
            })
            .collect()
    }

    /// Check if dylint is available
    pub fn is_available(&self) -> bool {
        let lint_libs = self.lint_libs.lock().unwrap();
        !lint_libs.is_empty()
    }
}
