use crate::core::dylint::{
    cache::DylintDetectorCache,
    compiler::DylintDetectorCompiler,
    scanner::DylintDetectorScanner,
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
}

impl DylintDetectorManager {
    pub fn new() -> Result<Self> {
        let cache = Arc::new(Mutex::new(DylintDetectorCache::new()?));

        Ok(Self {
            scanner: DylintDetectorScanner::new(),
            compiler: DylintDetectorCompiler::new(),
            cache,
            nightly_version: None,
        })
    }

    /// Set the workspace root
    pub fn set_workspace_root(&mut self, root: PathBuf) {
        self.scanner.set_workspace_root(root);
    }

    /// Pre-build all detectors (compile and cache them for future reuse)
    /// This can be called separately to build detectors before they're needed
    pub async fn prebuild_detectors(&mut self) -> Result<()> {
        // Get nightly version
        let nightly_version = DylintDetectorCompiler::get_nightly_version()
            .context("Failed to get nightly Rust version. Make sure nightly is installed.")?;

        self.nightly_version = Some(nightly_version.clone());
        info!("Pre-building dylint detectors with nightly: {}", nightly_version);

        // Scan for detectors
        let detectors = self.scanner.scan_detectors();

        if detectors.is_empty() {
            info!("No dylint detectors found in workspace");
            return Ok(());
        }

        info!("Pre-building {} dylint detector(s)...", detectors.len());

        // Build and cache each detector (but don't load yet)
        for detector in detectors {
            if let Err(e) = self.build_and_cache_detector(&detector, &nightly_version).await {
                warn!("Failed to pre-build detector {}: {}", detector.crate_name, e);
            }
        }

        Ok(())
    }

    /// Initialize and compile all dylint detectors (reuse cached builds if available)
    /// Returns the paths to compiled detector libraries
    pub async fn initialize(&mut self) -> Result<Vec<PathBuf>> {
        // Get nightly version
        let nightly_version = DylintDetectorCompiler::get_nightly_version()
            .context("Failed to get nightly Rust version. Make sure nightly is installed.")?;

        self.nightly_version = Some(nightly_version.clone());
        info!("[Workspace Dylint] Initializing dylint detectors with nightly: {}", nightly_version);

        // Scan for detectors
        let detectors = self.scanner.scan_detectors();

        if detectors.is_empty() {
            info!("[Workspace Dylint] No dylint detectors found in workspace");
            return Ok(Vec::new());
        }

        info!("[Workspace Dylint] Found {} dylint detector(s), compiling (reusing cached builds if available)...", detectors.len());

        // Compile each detector (will reuse cached builds) and collect paths
        let mut compiled_paths = Vec::new();
        for detector in detectors {
            match self.build_and_cache_detector(&detector, &nightly_version).await {
                Ok(path) => {
                    compiled_paths.push(path);
                }
                Err(e) => {
                    warn!("Failed to compile detector {}: {}", detector.crate_name, e);
                }
            }
        }

        info!("[Workspace Dylint] Successfully compiled {} detector(s)", compiled_paths.len());
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
            info!("Detector {} already cached, skipping build", detector.crate_name);
            return Ok(cached);
        }

        // Not cached - compile it
        drop(cache);
        info!("Building detector: {} with nightly {}", detector.crate_name, nightly_version);

        let compiled = self.compiler
            .compile_detector(detector, nightly_version)
            .await
            .context("Failed to compile detector")?;

        // Cache the compiled version for future reuse
        let cache = self.cache.lock().await;
        let cached_path = cache.cache_library(detector, nightly_version, &compiled)
            .context("Failed to cache compiled detector")?;

        info!("Built and cached detector for future reuse: {:?}", cached_path);
        Ok(cached_path)
    }


    /// Reload all detectors (useful after workspace changes)
    pub async fn reload(&mut self) -> Result<Vec<PathBuf>> {
        // Reinitialize (will reuse cache)
        self.initialize().await
    }

    /// Get the current nightly version
    pub fn nightly_version(&self) -> Option<&str> {
        self.nightly_version.as_deref()
    }
}

impl Default for DylintDetectorManager {
    fn default() -> Self {
        Self::new().expect("Failed to create DylintDetectorManager")
    }
}
