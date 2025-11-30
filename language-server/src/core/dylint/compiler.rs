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

    /// Compile a dylint detector crate with nightly Rust
    pub async fn compile_detector(
        &self,
        detector: &DylintDetectorInfo,
        nightly_version: &str,
    ) -> Result<PathBuf> {
        info!(
            "Compiling dylint detector {} with nightly {}",
            detector.crate_name, nightly_version
        );

        // Build the detector using cargo +nightly
        // Use debug mode for faster builds during development
        // TODO: Make this configurable or use release for production
        let output = TokioCommand::new("cargo")
            .arg("+nightly")
            .arg("build")
            // .arg("--release")  // Commented out for faster development builds
            .arg("--manifest-path")
            .arg(&detector.cargo_toml_path)
            .current_dir(&detector.crate_path)
            .output()
            .await
            .context("Failed to execute cargo build")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to compile detector: {}", stderr);
        }

        // Find the compiled library file
        let lib_path = self.find_compiled_library(&detector.crate_path, &detector.crate_name)?;

        info!("Successfully compiled detector to: {:?}", lib_path);
        Ok(lib_path)
    }

    /// Find the compiled library file (.so on Linux, .dylib on macOS, .dll on Windows)
    fn find_compiled_library(
        &self,
        crate_path: &Path,
        crate_name: &str,
    ) -> Result<PathBuf> {
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

    /// Get the current nightly Rust version
    pub fn get_nightly_version() -> Result<String> {
        let output = Command::new("rustc")
            .arg("+nightly")
            .arg("--version")
            .output()
            .context("Failed to execute rustc")?;

        if !output.status.success() {
            anyhow::bail!("Failed to get nightly version");
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
