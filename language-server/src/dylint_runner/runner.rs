use super::diagnostics::DylintDiagnostic;
use super::parser::parse_json_output;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use tokio::sync::{Mutex, oneshot};

#[derive(Debug)]
pub struct DylintRunner {
    /// Path to pre-compiled lint libraries (e.g., lints_compiled/macos-arm64/)
    #[allow(dead_code)]
    lint_libs_dir: PathBuf,

    /// List of lint library files to load
    lint_libs: Vec<PathBuf>,

    /// Cancellation support for running checks
    #[allow(dead_code)]
    current_check: Arc<Mutex<Option<oneshot::Sender<()>>>>,
}

impl DylintRunner {
    /// Initialize the runner with pre-compiled lints
    pub fn new(extension_path: &Path) -> Result<Self> {
        // 1. Detect platform
        let platform = Self::detect_platform()?;

        // 2. Find pre-compiled lints directory
        let lint_libs_dir = extension_path.join("lints_compiled").join(platform);

        if !lint_libs_dir.exists() {
            anyhow::bail!(
                "Lint libraries directory not found: {}. \
                Platform '{}' may not be supported.",
                lint_libs_dir.display(),
                platform
            );
        }

        // 3. Discover all .dylib/.so files in the directory
        let lint_libs = Self::discover_lint_libs(&lint_libs_dir)?;

        if lint_libs.is_empty() {
            anyhow::bail!(
                "No lint libraries found in {}. \
                Please build lints by running: cd lints && ./build_all_lints.sh",
                lint_libs_dir.display()
            );
        }

        eprintln!(
            "🚀 Dylint runner initialized with {} lints from {}",
            lint_libs.len(),
            lint_libs_dir.display()
        );

        Ok(Self {
            lint_libs_dir,
            lint_libs,
            current_check: Arc::new(Mutex::new(None)),
        })
    }

    /// Run lints on a workspace (called on file save, debounced)
    pub async fn run_lints(&self, workspace_path: &Path) -> Result<Vec<DylintDiagnostic>> {
        // Cancel any running check
        {
            let mut guard = self.current_check.lock().await;
            if let Some(cancel) = guard.take() {
                let _ = cancel.send(());
            }
        }

        // Create new cancellation channel
        let (cancel_tx, mut cancel_rx) = oneshot::channel::<()>();
        *self.current_check.lock().await = Some(cancel_tx);

        eprintln!(
            "🔍 Running dylint lints on workspace: {}",
            workspace_path.display()
        );
        eprintln!("   Lints: {:?}", self.lint_libs);

        // Check if workspace has Cargo.toml
        let cargo_toml = workspace_path.join("Cargo.toml");
        if !cargo_toml.exists() {
            anyhow::bail!(
                "No Cargo.toml found in workspace: {}. \
                Dylint requires a Rust project with Cargo.toml.",
                workspace_path.display()
            );
        }

        // Build PATH with cargo bin directories
        let current_path = std::env::var("PATH").unwrap_or_default();
        let home = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;

        let cargo_bin = home.join(".cargo").join("bin");
        let new_path = format!(
            "{}:/usr/local/bin:/usr/bin:{}",
            cargo_bin.display(),
            current_path
        );

        eprintln!("   Using PATH: {}", new_path);

        // Detect toolchain from lint's rust-toolchain file
        // We need to use the same nightly toolchain that the lints were built with
        let toolchain = Self::detect_lint_toolchain()?;
        eprintln!("   Using toolchain: {}", toolchain);

        // Get dylint-driver path
        let toolchain_target = format!("{}-aarch64-apple-darwin", toolchain); // TODO: make platform-aware
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

        eprintln!("   Using dylint-driver: {}", dylint_driver.display());

        // Build DYLINT_LIBS JSON array with absolute paths
        let dylint_libs_json = serde_json::to_string(
            &self
                .lint_libs
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
        )?;

        eprintln!("   DYLINT_LIBS: {}", dylint_libs_json);

        // Use cargo check with RUSTC_WORKSPACE_WRAPPER (simpler approach from example)
        // Use --workspace to ONLY lint workspace members, NOT dependencies
        let child = tokio::process::Command::new("cargo")
            .arg(format!("+{}", toolchain))
            .args(&["check", "--workspace", "--message-format=json"])
            .current_dir(workspace_path)
            .env("PATH", new_path)
            .env("RUSTC_WORKSPACE_WRAPPER", &dylint_driver)
            .env("DYLINT_LIBS", dylint_libs_json)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context(format!(
                "Failed to spawn cargo check in {}. Make sure cargo is installed and in PATH.",
                workspace_path.display()
            ))?;

        // Get the child ID for cancellation (before moving child)
        let child_id = child.id();

        // Wait for either completion or cancellation
        let output = tokio::select! {
            result = child.wait_with_output() => {
                result.context("cargo check failed")?
            }

            _ = &mut cancel_rx => {
                // Cancelled - try to kill the process using the PID
                if let Some(pid) = child_id {
                    #[cfg(unix)]
                    {
                        use std::process::Command as StdCommand;
                        let _ = StdCommand::new("kill").arg(pid.to_string()).status();
                    }
                    #[cfg(windows)]
                    {
                        use std::process::Command as StdCommand;
                        let _ = StdCommand::new("taskkill")
                            .args(&["/F", "/PID", &pid.to_string()])
                            .status();
                    }
                }
                anyhow::bail!("Lint check cancelled");
            }
        };

        // Extract lint names from loaded libraries
        let stdout = String::from_utf8_lossy(&output.stdout);
        // e.g., "libaddition_detector@...dylib" -> "addition_detector"
        let lint_codes: Vec<String> = self
            .lint_libs
            .iter()
            .filter_map(|path| {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .and_then(|s| s.split('@').next()) // Split at @ to remove toolchain suffix
                    .map(|s| s.strip_prefix("lib").unwrap_or(s).to_string())
            })
            .collect();

        // Parse JSON output - only accept diagnostics from our lints
        let diagnostics = parse_json_output(&stdout, &lint_codes)?;

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

    /// Detect the Rust toolchain from the lint's rust-toolchain file
    /// All lints should be built with the same toolchain
    fn detect_lint_toolchain() -> Result<String> {
        // Look for rust-toolchain file in any lint directory under lints/
        let lints_dir = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|p| p.to_path_buf())) // extension/bin/
            .and_then(|bin| bin.parent().map(|p| p.to_path_buf())) // extension/
            .and_then(|ext| ext.parent().map(|p| p.to_path_buf())) // project root/
            .map(|root| root.join("lints"))
            .ok_or_else(|| anyhow::anyhow!("Could not determine lints directory"))?;

        if let Ok(entries) = std::fs::read_dir(&lints_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let rust_toolchain = entry.path().join("rust-toolchain");
                if rust_toolchain.exists() {
                    if let Ok(content) = std::fs::read_to_string(&rust_toolchain) {
                        // Parse TOML to extract channel
                        for line in content.lines() {
                            if line.trim().starts_with("channel") {
                                if let Some(channel) = line.split('"').nth(1) {
                                    return Ok(channel.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }

        // Fallback to default nightly
        Ok("nightly-2025-08-07".to_string())
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

    /// Get list of loaded lint names (for logging/debugging)
    pub fn loaded_lints(&self) -> Vec<String> {
        self.lint_libs
            .iter()
            .filter_map(|p| {
                p.file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
            })
            .collect()
    }
}
