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

    /// Get the cache directory path with version
    fn get_cache_directory() -> Result<PathBuf> {
        let cache_base = dirs::cache_dir().context("Failed to get cache directory")?;

        // Include extension version in cache path so each version gets its own cache
        // This automatically invalidates cache on version updates
        let version = Self::get_extension_version().unwrap_or_else(|_| "unknown".to_string());

        Ok(cache_base
            .join("solana-vscode")
            .join(format!("dylint-detectors-v{}", version)))
    }

    /// Read extension version from package.json
    fn get_extension_version() -> Result<String> {
        // Try to find package.json relative to the language server
        // The structure is: extension/package.json and language-server/
        let current_exe = std::env::current_exe()?;
        let mut search_path = current_exe.parent();

        // Search up the directory tree for package.json
        while let Some(path) = search_path {
            // Check ../extension/package.json (language-server sibling)
            let package_json = path
                .parent()
                .and_then(|p| Some(p.join("extension").join("package.json")));

            if let Some(pkg_path) = package_json {
                if pkg_path.exists() {
                    let content =
                        fs::read_to_string(&pkg_path).context("Failed to read package.json")?;

                    // Simple version extraction (avoiding serde_json dependency)
                    if let Some(version_line) = content
                        .lines()
                        .find(|line| line.trim().starts_with("\"version\""))
                    {
                        if let Some(version) = version_line.split(':').nth(1).and_then(|v| {
                            v.trim()
                                .trim_matches(',')
                                .trim_matches('"')
                                .strip_prefix("")
                        }) {
                            return Ok(version.trim_matches('"').to_string());
                        }
                    }
                }
            }

            search_path = path.parent();
        }

        // Fallback to language server version if package.json not found
        Ok(env!("CARGO_PKG_VERSION").to_string())
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
    pub fn clear_all(&self) -> Result<()> {
        if self.cache_dir.exists() {
            for entry in fs::read_dir(&self.cache_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() {
                    fs::remove_file(&path).context("Failed to remove cached file")?;
                    info!("Removed cached detector: {:?}", path);
                }
            }
            info!("Cleared all cached detectors");
        }
        Ok(())
    }

    /// Get the cache directory path (for external access)
    pub fn cache_dir(&self) -> &PathBuf {
        &self.cache_dir
    }
}
