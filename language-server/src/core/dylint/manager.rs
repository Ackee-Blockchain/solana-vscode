use crate::core::dylint::{
    cache::DylintDetectorCache, compiler::DylintDetectorCompiler, scanner::DylintDetectorScanner,
};
use anyhow::{Context, Result};
use log::{info, warn};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Manager for dylint detectors - handles scanning, compilation, and caching
/// Compiled detectors are added to dylint_runner which runs them via cargo +nightly dylint
#[derive(Debug)]
pub struct DylintDetectorManager {
    scanner: DylintDetectorScanner,
    compiler: DylintDetectorCompiler,
    cache: Arc<Mutex<DylintDetectorCache>>,
    nightly_version: Option<String>,
    /// Whether detectors have been initialized (compiled/cached)
    initialized: bool,
    /// Cached list of compiled detector paths
    compiled_paths: Vec<PathBuf>,
}

impl DylintDetectorManager {
    pub fn new() -> Result<Self> {
        let cache = Arc::new(Mutex::new(DylintDetectorCache::new()?));

        Ok(Self {
            scanner: DylintDetectorScanner::new(),
            compiler: DylintDetectorCompiler::new(),
            cache,
            nightly_version: None,
            initialized: false,
            compiled_paths: Vec::new(),
        })
    }

    /// Check if nightly Rust is available
    pub fn check_nightly_available() -> bool {
        DylintDetectorCompiler::is_nightly_available()
    }

    /// Check if dylint-driver is available
    pub fn check_dylint_driver_available() -> bool {
        DylintDetectorCompiler::is_dylint_driver_available()
    }

    /// Check if detectors have been initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Set the extension path (where bundled detectors are located)
    pub fn set_extension_path(&mut self, extension_path: PathBuf) {
        self.scanner.set_extension_path(extension_path);
    }

    /// Initialize and compile all dylint detectors (reuse cached builds if available)
    /// Returns the paths to compiled detector libraries
    /// This is the main initialization method called on first save
    pub async fn initialize(&mut self) -> Result<Vec<PathBuf>> {
        // If already initialized, return cached paths
        if self.initialized {
            info!("[Extension Dylint] Detectors already initialized, returning cached paths");
            return Ok(self.compiled_paths.clone());
        }

        // Get nightly version
        let nightly_version = DylintDetectorCompiler::get_nightly_version()
            .context("Failed to get nightly Rust version. Make sure nightly is installed.")?;

        self.nightly_version = Some(nightly_version.clone());
        info!(
            "[Extension Dylint] Initializing dylint detectors with nightly: {}",
            nightly_version
        );

        // Clean up old cache directories from previous versions
        self.cleanup_old_cache_directories().await?;

        // Scan for detectors in extension/detectors/
        let detectors = self.scanner.scan_detectors();

        if detectors.is_empty() {
            info!("[Extension Dylint] No dylint detectors found in extension");
            self.initialized = true; // Mark as initialized even if empty
            return Ok(Vec::new());
        }

        info!(
            "[Extension Dylint] Found {} dylint detector(s), checking cache or compiling...",
            detectors.len()
        );

        // Compile each detector (will reuse cached builds) and collect paths
        let mut compiled_paths = Vec::new();
        for detector in detectors {
            match self
                .build_and_cache_detector(&detector, &nightly_version)
                .await
            {
                Ok(path) => {
                    compiled_paths.push(path);
                }
                Err(e) => {
                    warn!("Failed to compile detector {}: {}", detector.crate_name, e);
                }
            }
        }

        // Mark as initialized and cache paths
        self.initialized = true;
        self.compiled_paths = compiled_paths.clone();

        info!(
            "[Extension Dylint] Successfully initialized {} detector(s)",
            compiled_paths.len()
        );
        Ok(compiled_paths)
    }

    /// Build and cache a detector (without loading it)
    async fn build_and_cache_detector(
        &self,
        detector: &crate::core::dylint::scanner::DylintDetectorInfo,
        nightly_version: &str,
    ) -> Result<PathBuf> {
        let cache = self.cache.lock().await;

        // Check if already cached - if so, just return the cached path
        if let Some(cached) = cache.get_cached_library(detector, nightly_version) {
            info!(
                "Detector {} already cached, skipping build",
                detector.crate_name
            );
            return Ok(cached);
        }

        // Not cached - compile it
        drop(cache);
        info!(
            "Building detector: {} with nightly {}",
            detector.crate_name, nightly_version
        );

        let compiled = self
            .compiler
            .compile_detector(detector, nightly_version)
            .await
            .context("Failed to compile detector")?;

        // Cache the compiled version for future reuse
        let cache = self.cache.lock().await;
        let cached_path = cache
            .cache_library(detector, nightly_version, &compiled)
            .context("Failed to cache compiled detector")?;

        info!(
            "Built and cached detector for future reuse: {:?}",
            cached_path
        );
        Ok(cached_path)
    }

    /// Clean up cache directories from old extension versions
    async fn cleanup_old_cache_directories(&self) -> Result<()> {
        let cache = self.cache.lock().await;
        let current_cache_dir = cache.cache_dir();

        // Get the parent directory (solana-vscode/) and check if it exists
        if let Some(parent_dir) = current_cache_dir.parent().filter(|p| p.exists()) {
            // Iterate through all directories in solana-vscode/
            if let Ok(entries) = std::fs::read_dir(parent_dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();

                    // Check if it's a dylint-detectors directory but not the current one
                    if path.is_dir()
                        && path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .map(|n| n.starts_with("dylint-detectors"))
                            .unwrap_or(false)
                        && path != *current_cache_dir
                    {
                        // Delete old cache directory
                        if let Err(e) = std::fs::remove_dir_all(&path) {
                            warn!("Failed to remove old cache directory {:?}: {}", path, e);
                        } else {
                            info!("Removed old cache directory: {:?}", path);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

impl Default for DylintDetectorManager {
    fn default() -> Self {
        Self::new().expect("Failed to create DylintDetectorManager")
    }
}
