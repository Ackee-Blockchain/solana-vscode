use std::collections::HashMap;
use tower_lsp::lsp_types::Diagnostic;
use crate::core::detector::Detector;
use crate::core::detector_config::DetectorConfig;

/// Registry that manages all security detectors
pub struct DetectorRegistry {
    detectors: Vec<Box<dyn Detector>>,
    configs: HashMap<String, DetectorConfig>,
}

impl std::fmt::Debug for DetectorRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DetectorRegistry")
            .field("detector_count", &self.detectors.len())
            .field("configs", &self.configs)
            .finish()
    }
}

impl DetectorRegistry {
    /// Create a new detector registry
    pub fn new() -> Self {
        Self {
            detectors: Vec::new(),
            configs: HashMap::new(),
        }
    }

    /// Register a detector with the registry
    pub fn register<D: Detector + 'static>(&mut self, detector: D) {
        let id = detector.id().to_string();
        self.configs.insert(id, DetectorConfig::default());
        self.detectors.push(Box::new(detector));
    }

    /// Configure a specific detector
    pub fn configure(&mut self, detector_id: &str, config: DetectorConfig) {
        self.configs.insert(detector_id.to_string(), config);
    }

    /// Disable a specific detector
    pub fn disable(&mut self, detector_id: &str) {
        if let Some(config) = self.configs.get_mut(detector_id) {
            config.enabled = false;
        }
    }

    /// Enable a specific detector
    pub fn enable(&mut self, detector_id: &str) {
        if let Some(config) = self.configs.get_mut(detector_id) {
            config.enabled = true;
        }
    }

    /// Run all enabled detectors on the given content
    pub fn analyze(&mut self, content: &str) -> Vec<Diagnostic> {
        let mut all_diagnostics = Vec::new();

        for detector in &mut self.detectors {
            let config = self.configs.get(detector.id()).cloned().unwrap_or_default();

            if !config.enabled || !detector.should_run(content) {
                continue;
            }

            let mut diagnostics = detector.analyze(content);

            // Apply severity override if configured
            if let Some(severity_override) = config.severity_override {
                for diagnostic in &mut diagnostics {
                    diagnostic.severity = Some(severity_override);
                }
            }

            all_diagnostics.extend(diagnostics);
        }

        all_diagnostics
    }

    /// Get information about all registered detectors
    pub fn list_detectors(&self) -> Vec<DetectorInfo> {
        self.detectors
            .iter()
            .map(|detector| {
                let config = self.configs.get(detector.id()).cloned().unwrap_or_default();

                DetectorInfo {
                    id: detector.id().to_string(),
                    name: detector.name().to_string(),
                    description: detector.description().to_string(),
                    enabled: config.enabled,
                    default_severity: detector.default_severity(),
                }
            })
            .collect()
    }

    /// Get the number of registered detectors
    pub fn count(&self) -> usize {
        self.detectors.len()
    }

    /// Get the number of enabled detectors
    pub fn enabled_count(&self) -> usize {
        self.detectors
            .iter()
            .filter(|detector| {
                self.configs
                    .get(detector.id())
                    .map(|config| config.enabled)
                    .unwrap_or(true)
            })
            .count()
    }
}

impl Default for DetectorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Information about a detector
#[derive(Debug, Clone)]
pub struct DetectorInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub default_severity: tower_lsp::lsp_types::DiagnosticSeverity,
}

/// Builder for creating and configuring a detector registry
pub struct DetectorRegistryBuilder {
    registry: DetectorRegistry,
}

impl DetectorRegistryBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            registry: DetectorRegistry::new(),
        }
    }

    /// Add a detector to the registry
    pub fn with_detector<D: Detector + 'static>(mut self, detector: D) -> Self {
        self.registry.register(detector);
        self
    }

    /// Configure a detector
    pub fn with_config(mut self, detector_id: &str, config: DetectorConfig) -> Self {
        self.registry.configure(detector_id, config);
        self
    }

    /// Build the registry
    pub fn build(self) -> DetectorRegistry {
        self.registry
    }
}

impl Default for DetectorRegistryBuilder {
    fn default() -> Self {
        Self::new()
    }
}
