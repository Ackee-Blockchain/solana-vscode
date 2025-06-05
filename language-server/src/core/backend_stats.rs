use crate::core::{DetectorInfo, DetectorRegistry};

/// Statistics about the detector system
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct DetectorStats {
    pub total_detectors: usize,
    pub enabled_detectors: usize,
}

/// Backend management functionality
pub struct BackendManager;

impl BackendManager {
    /// Get information about all registered detectors
    #[allow(dead_code)]
    pub async fn list_detectors(registry: &DetectorRegistry) -> Vec<DetectorInfo> {
        registry.list_detectors()
    }

    /// Enable or disable a specific detector
    #[allow(dead_code)]
    pub async fn set_detector_enabled(
        registry: &mut DetectorRegistry,
        detector_id: &str,
        enabled: bool,
    ) {
        if enabled {
            registry.enable(detector_id);
        } else {
            registry.disable(detector_id);
        }
    }

    /// Get detector statistics
    #[allow(dead_code)]
    pub async fn get_detector_stats(registry: &DetectorRegistry) -> DetectorStats {
        DetectorStats {
            total_detectors: registry.count(),
            enabled_detectors: registry.enabled_count(),
        }
    }
}
