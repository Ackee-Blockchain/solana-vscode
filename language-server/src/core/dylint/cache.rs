use crate::core::dylint::constants::REQUIRED_NIGHTLY_VERSION;
use crate::core::dylint::scanner::DylintDetectorInfo;
use anyhow::{Context, Result};
use log::{debug, info};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

/// Cache manager for compiled dylint detectors
pub struct DylintDetectorCache {
    cache_dir: PathBuf,
}

impl std::fmt::Debug for DylintDetectorCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DylintDetectorCache")
            .field("cache_dir", &self.cache_dir)
            .finish()
    }
}

impl DylintDetectorCache {
    /// Create a new cache manager
    pub fn new() -> Result<Self> {
        let cache_dir = Self::get_cache_directory()?;

        // Create cache directory if it doesn't exist
        if !cache_dir.exists() {
            fs::create_dir_all(&cache_dir).context("Failed to create cache directory")?;
            info!("Created cache directory: {:?}", cache_dir);
        }

        Ok(Self { cache_dir })
    }

    /// Get the cache directory path
    fn get_cache_directory() -> Result<PathBuf> {
        let cache_base = dirs::cache_dir().context("Failed to get cache directory")?;

        Ok(cache_base.join("solana-vscode").join("dylint-detectors"))
    }

    /// Get the cache key for a detector and nightly version
    fn get_cache_key(detector: &DylintDetectorInfo, nightly_version: &str) -> String {
        // Create a hash of the detector path and nightly version
        let mut hasher = DefaultHasher::new();
        detector.crate_path.hash(&mut hasher);
        detector.crate_name.hash(&mut hasher);
        nightly_version.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }

    /// Get the cached library path for a detector
    pub fn get_cached_library(
        &self,
        detector: &DylintDetectorInfo,
        nightly_version: &str,
    ) -> Option<PathBuf> {
        // Try new format first (with nightly version in filename)
        let extension = if cfg!(target_os = "macos") {
            "dylib"
        } else if cfg!(target_os = "windows") {
            "dll"
        } else {
            "so"
        };
        
        // Always use the extension's required nightly version
        let platform = std::env::consts::ARCH;
        let os = match std::env::consts::OS {
            "macos" => "apple-darwin",
            "linux" => "unknown-linux-gnu",
            "windows" => "pc-windows-msvc",
            _ => "unknown",
        };
        
        let filename = format!(
            "lib{}@{}-{}-{}.{}",
            detector.crate_name.replace("-", "_"),
            REQUIRED_NIGHTLY_VERSION,
            platform,
            os,
            extension
        );

        let lib_path = self.cache_dir.join(&filename);
        if lib_path.exists() {
            debug!("Found cached library (new format): {:?}", lib_path);
            return Some(lib_path);
        }

        // Fallback: try old hash-based format
        let cache_key = Self::get_cache_key(detector, nightly_version);
        let cached_path = self.cache_dir.join(&cache_key);
        let lib_path = cached_path.with_extension(extension);
        if lib_path.exists() {
            debug!("Found cached library (old format): {:?}", lib_path);
            return Some(lib_path);
        }

        None
    }

    /// Store a compiled library in the cache
    /// The filename includes the detector name and nightly version for easy identification
    pub fn cache_library(
        &self,
        detector: &DylintDetectorInfo,
        nightly_version: &str,
        compiled_lib: &Path,
    ) -> Result<PathBuf> {
        let extension = compiled_lib
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("so");
        
        // Always use the extension's required nightly version
        // Create filename: lib<detector_name>@<nightly_version>-<platform>.<ext>
        // This format matches the pre-compiled lints and allows dylint runner to detect toolchain
        let platform = std::env::consts::ARCH;
        let os = match std::env::consts::OS {
            "macos" => "apple-darwin",
            "linux" => "unknown-linux-gnu",
            "windows" => "pc-windows-msvc",
            _ => "unknown",
        };
        
        let filename = format!(
            "lib{}@{}-{}-{}.{}",
            detector.crate_name.replace("-", "_"),
            REQUIRED_NIGHTLY_VERSION,
            platform,
            os,
            extension
        );

        let cached_path = self.cache_dir.join(filename);

        // Copy the compiled library to cache
        fs::copy(compiled_lib, &cached_path).context("Failed to copy library to cache")?;

        info!("Cached library to: {:?}", cached_path);
        Ok(cached_path)
    }

    /// Check if a cached library exists and is valid
    pub fn is_cached(&self, detector: &DylintDetectorInfo, nightly_version: &str) -> bool {
        self.get_cached_library(detector, nightly_version).is_some()
    }

    /// Clear the cache for a specific detector
    pub fn clear_cache(&self, detector: &DylintDetectorInfo, nightly_version: &str) -> Result<()> {
        if let Some(cached_path) = self.get_cached_library(detector, nightly_version) {
            fs::remove_file(&cached_path).context("Failed to remove cached library")?;
            info!("Cleared cache for detector: {}", detector.crate_name);
        }
        Ok(())
    }

    /// Clear all cached detectors
    pub fn clear_all_cache(&self) -> Result<()> {
        if self.cache_dir.exists() {
            fs::remove_dir_all(&self.cache_dir).context("Failed to clear cache directory")?;
            fs::create_dir_all(&self.cache_dir).context("Failed to recreate cache directory")?;
            info!("Cleared all cached detectors");
        }
        Ok(())
    }
}
