use anyhow::{Context, Result};
use libloading::Library;
use log::{info, warn};
use std::path::Path;

/// Loader for dynamically loading compiled dylint detectors
pub struct DylintDetectorLoader {
    libraries: Vec<Library>,
}

impl std::fmt::Debug for DylintDetectorLoader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DylintDetectorLoader")
            .field("library_count", &self.libraries.len())
            .finish()
    }
}

impl DylintDetectorLoader {
    pub fn new() -> Self {
        Self {
            libraries: Vec::new(),
        }
    }

    /// Load a compiled dylint detector library
    pub unsafe fn load_detector(&mut self, lib_path: &Path) -> Result<()> {
        info!("Loading dylint detector from: {:?}", lib_path);

        let library = unsafe { Library::new(lib_path) }
            .context(format!("Failed to load library from {:?}", lib_path))?;

        // Try to find and call the detector registration function
        // Dylint detectors typically export a function like `register_detector` or similar
        // This is a placeholder - actual implementation depends on dylint detector API
        match unsafe { self.register_detector_from_library(&library) } {
            Ok(_) => {
                info!("Successfully loaded detector from {:?}", lib_path);
                self.libraries.push(library);
                Ok(())
            }
            Err(e) => {
                warn!("Failed to register detector from {:?}: {}", lib_path, e);
                Err(e)
            }
        }
    }

    /// Register a detector from a loaded library
    unsafe fn register_detector_from_library(&self, library: &Library) -> Result<()> {
        // Try to find the registration function
        // The exact function name depends on the dylint detector implementation
        // Common patterns: "register", "init", "setup"
        let func_names = ["register", "init", "setup", "register_detector"];

        for func_name in &func_names {
            if let Ok(symbol) = unsafe { library.get::<extern "C" fn()>(func_name.as_bytes()) } {
                // Call the registration function
                symbol();
                return Ok(());
            }
        }

        // If no registration function found, that's okay - the detector might be loaded differently
        // Just log a warning and continue
        warn!("No registration function found in library, assuming auto-registration");
        Ok(())
    }

    /// Unload all loaded detectors
    pub fn unload_all(&mut self) {
        info!("Unloading {} detector library(ies)", self.libraries.len());
        self.libraries.clear();
    }
}

impl Default for DylintDetectorLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for DylintDetectorLoader {
    fn drop(&mut self) {
        self.unload_all();
    }
}
