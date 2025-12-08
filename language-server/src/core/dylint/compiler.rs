use crate::core::dylint::constants::REQUIRED_NIGHTLY_VERSION;
use crate::core::dylint::scanner::DylintDetectorInfo;
use anyhow::{Context, Result};
use log::info;
use std::path::Path;
use std::process::Command;
use tokio::process::Command as TokioCommand;

/// Compiler for dylint detector crates
pub struct DylintDetectorCompiler;

impl std::fmt::Debug for DylintDetectorCompiler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DylintDetectorCompiler").finish()
    }
}

impl DylintDetectorCompiler {
    pub fn new() -> Self {
        Self
    }

    /// Compile a dylint detector crate with the extension's required nightly Rust version
    /// All detectors must be compatible with REQUIRED_NIGHTLY_VERSION
    pub async fn compile_detector(
        &self,
        detector: &DylintDetectorInfo,
        _nightly_version: &str,
    ) -> Result<PathBuf> {
        info!(
            "Compiling dylint detector {} with required nightly {}",
            detector.crate_name, REQUIRED_NIGHTLY_VERSION
        );

        // Always use the extension's required nightly version
        let toolchain_arg = format!("+{}", REQUIRED_NIGHTLY_VERSION);

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

        // Build the detector using cargo with the required nightly
        // Use debug mode for faster builds during development
        let output = TokioCommand::new("cargo")
            .arg(&toolchain_arg)
            .arg("build")
            // .arg("--release")  // Commented out for faster development builds
            .arg("--manifest-path")
            .arg(&detector.cargo_toml_path)
            .current_dir(&detector.crate_path)
            .env("PATH", new_path)
            .output()
            .await
            .context("Failed to execute cargo build")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            log::error!("Detector compilation failed!");
            log::error!("stdout: {}", stdout);
            log::error!("stderr: {}", stderr);
            anyhow::bail!("Failed to compile detector: {}", stderr);
        }

        // Find the compiled library file
        let lib_path = self.find_compiled_library(&detector.crate_path, &detector.crate_name)?;

        info!("Successfully compiled detector to: {:?}", lib_path);
        Ok(lib_path)
    }

    /// Find the compiled library file (.so on Linux, .dylib on macOS, .dll on Windows)
    fn find_compiled_library(&self, crate_path: &Path, crate_name: &str) -> Result<PathBuf> {
        // Check both debug and release directories (debug is default now)
        let debug_dir = crate_path.join("target").join("debug");
        let release_dir = crate_path.join("target").join("release");

        // Try debug first (faster builds), then release
        let target_dir = if debug_dir.exists() {
            debug_dir
        } else {
            release_dir
        };

        // Try different library extensions
        let extensions = if cfg!(target_os = "macos") {
            vec!["dylib"]
        } else if cfg!(target_os = "windows") {
            vec!["dll"]
        } else {
            vec!["so"]
        };

        for ext in extensions {
            let lib_name = format!("lib{}.{}", crate_name.replace("-", "_"), ext);
            let lib_path = target_dir.join(&lib_name);
            if lib_path.exists() {
                return Ok(lib_path);
            }

            // Also try without lib prefix (Windows)
            let lib_name = format!("{}.{}", crate_name.replace("-", "_"), ext);
            let lib_path = target_dir.join(&lib_name);
            if lib_path.exists() {
                return Ok(lib_path);
            }
        }

        anyhow::bail!(
            "Could not find compiled library for {} in {:?}",
            crate_name,
            target_dir
        )
    }

    /// Check if the required nightly Rust version is available
    pub fn is_nightly_available() -> bool {
        use log::{debug, warn};

        debug!("[Nightly Check] Attempting to check nightly availability...");

        // Build PATH with cargo bin directories (same as dylint runner)
        let current_path = std::env::var("PATH").unwrap_or_default();
        let home = match dirs::home_dir() {
            Some(h) => h,
            None => {
                warn!("[Nightly Check] Could not determine home directory");
                return false;
            }
        };

        let cargo_bin = home.join(".cargo").join("bin");
        let rustup_bin = home.join(".rustup").join("toolchains");
        let new_path = format!(
            "{}:{}:/usr/local/bin:/usr/bin:{}",
            cargo_bin.display(),
            rustup_bin.display(),
            current_path
        );

        debug!("[Nightly Check] Using PATH: {}", new_path);
        debug!(
            "[Nightly Check] Checking for required nightly: {}",
            REQUIRED_NIGHTLY_VERSION
        );

        match Command::new("rustc")
            .arg(format!("+{}", REQUIRED_NIGHTLY_VERSION))
            .arg("--version")
            .env("PATH", new_path)
            .output()
        {
            Ok(output) => {
                let success = output.status.success();
                if success {
                    let version = String::from_utf8_lossy(&output.stdout);
                    debug!("[Nightly Check] Success! Version: {}", version.trim());
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    warn!("[Nightly Check] Command failed. stderr: {}", stderr);
                }
                success
            }
            Err(e) => {
                warn!("[Nightly Check] Failed to execute rustc: {}", e);
                false
            }
        }
    }

    /// Get the required nightly Rust version (returns the constant)
    /// This ensures all detectors use the same nightly version
    pub fn get_nightly_version() -> Result<String> {
        // Simply return the required version - we don't query rustc
        Ok(REQUIRED_NIGHTLY_VERSION.to_string())
    }

    /// Get the actual installed nightly version info (for logging)
    pub fn get_installed_nightly_info() -> Result<String> {
        // Build PATH with cargo bin directories (same as nightly check)
        let current_path = std::env::var("PATH").unwrap_or_default();
        let home = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;

        let cargo_bin = home.join(".cargo").join("bin");
        let rustup_bin = home.join(".rustup").join("toolchains");
        let new_path = format!(
            "{}:{}:/usr/local/bin:/usr/bin:{}",
            cargo_bin.display(),
            rustup_bin.display(),
            current_path
        );

        let output = Command::new("rustc")
            .arg(format!("+{}", REQUIRED_NIGHTLY_VERSION))
            .arg("--version")
            .env("PATH", new_path)
            .output()
            .context("Failed to execute rustc")?;

        if !output.status.success() {
            anyhow::bail!("Failed to get installed nightly info");
        }

        let version = String::from_utf8_lossy(&output.stdout);
        // Extract version string (e.g., "rustc 1.75.0-nightly (abc123 2024-01-01)")
        let version = version.trim();
        Ok(version.to_string())
    }
}

impl Default for DylintDetectorCompiler {
    fn default() -> Self {
        Self::new()
    }
}

use std::path::PathBuf;
