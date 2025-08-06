use crate::core::detectors::clippy_analyzer::ClippyAnalyzer;
use crate::core::detectors::detector::{
    ClippyAnalysisContext, ClippyDetector, DetectorType, DetectorWrapper, SynDetector,
};
use crate::core::detectors::detector_config::DetectorConfig;
use std::collections::HashMap;
use std::path::PathBuf;
use tower_lsp::lsp_types::Diagnostic;

/// Registry that manages all security detectors (both syn-based and clippy-style)
pub struct DetectorRegistry {
    /// All detectors stored in wrappers
    detectors: Vec<DetectorWrapper>,
    /// Clippy-style analyzer for background analysis
    clippy_analyzer: ClippyAnalyzer,
    /// Configuration for each detector
    configs: HashMap<String, DetectorConfig>,
}

impl std::fmt::Debug for DetectorRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let syn_count = self.detectors.iter().filter(|d| d.is_syn()).count();
        let clippy_count = self.detectors.iter().filter(|d| d.is_clippy()).count();

        f.debug_struct("DetectorRegistry")
            .field("syn_detector_count", &syn_count)
            .field("clippy_detector_count", &clippy_count)
            .field("total_detectors", &self.detectors.len())
            .field("configs", &self.configs)
            .finish()
    }
}

impl DetectorRegistry {
    /// Create a new detector registry
    pub fn new() -> Self {
        Self {
            detectors: Vec::new(),
            clippy_analyzer: ClippyAnalyzer::new(),
            configs: HashMap::new(),
        }
    }

    /// Register a syn detector with the registry
    pub fn register_syn<D: SynDetector + 'static>(&mut self, detector: D) {
        let id = detector.id().to_string();
        self.configs.insert(id, DetectorConfig::default());
        self.detectors.push(DetectorWrapper::new_syn(detector));
    }

    /// Register a clippy-style detector specifically
    pub fn register_clippy_detector<D: ClippyDetector + 'static>(&mut self, detector: D) {
        let id = detector.id().to_string();
        self.configs.insert(id, DetectorConfig::default());
        self.detectors.push(DetectorWrapper::new_clippy(detector));
    }

    /// Set workspace root for clippy analysis
    pub fn set_workspace_root(&mut self, root: PathBuf) {
        self.clippy_analyzer.set_workspace_root(root);
    }

    /// Configure a specific detector
    #[allow(dead_code)]
    pub fn configure(&mut self, detector_id: &str, config: DetectorConfig) {
        self.configs.insert(detector_id.to_string(), config);
    }

    /// Disable a specific detector
    #[allow(dead_code)]
    pub fn disable(&mut self, detector_id: &str) {
        if let Some(config) = self.configs.get_mut(detector_id) {
            config.enabled = false;
        }
    }

    /// Enable a specific detector
    #[allow(dead_code)]
    pub fn enable(&mut self, detector_id: &str) {
        if let Some(config) = self.configs.get_mut(detector_id) {
            config.enabled = true;
        }
    }

    /// Run immediate syn-based analysis (fast)
    pub fn analyze_immediate(
        &mut self,
        content: &str,
        file_path: Option<&PathBuf>,
    ) -> Vec<Diagnostic> {
        let mut all_diagnostics = Vec::new();

        for detector in &mut self.detectors {
            let config = self.configs.get(detector.id()).cloned().unwrap_or_default();

            if !config.enabled {
                continue;
            }

            // Only run syn detectors for immediate analysis
            if detector.is_syn() {
                let mut diagnostics = detector.analyze_syn(content, file_path);

                // Apply severity override if configured
                if let Some(severity_override) = config.severity_override {
                    for diagnostic in &mut diagnostics {
                        diagnostic.severity = Some(severity_override);
                    }
                }

                all_diagnostics.extend(diagnostics);
            }
        }

        all_diagnostics
    }

    /// Run background clippy-style analysis (comprehensive but slower)
    pub async fn analyze_comprehensive(
        &mut self,
        file_path: &PathBuf,
        content: &str,
    ) -> Vec<Diagnostic> {
        let mut all_diagnostics = Vec::new();

        // Create analysis context
        let context = ClippyAnalysisContext {
            file_path: file_path.clone(),
            source_code: content.to_string(),
            compilation_successful: true, // We'll assume compilation is successful for now
        };

        // Run clippy detectors
        for detector in &mut self.detectors {
            if detector.is_clippy() {
                let config = self.configs.get(detector.id()).cloned().unwrap_or_default();

                if !config.enabled {
                    continue;
                }

                let mut diagnostics = detector.analyze_clippy(&context);

                // Apply severity override if configured
                if let Some(severity_override) = config.severity_override {
                    for diagnostic in &mut diagnostics {
                        diagnostic.severity = Some(severity_override);
                    }
                }

                all_diagnostics.extend(diagnostics);
            }
        }

        all_diagnostics
    }

    /// Invalidate clippy cache for a file
    pub async fn invalidate_cache(&self, file_path: &PathBuf) {
        self.clippy_analyzer.clear_cache(file_path).await;
    }

    /// Clear all caches
    pub async fn clear_cache(&self) {
        self.clippy_analyzer.clear_all_cache().await;
    }

    /// Get information about all registered detectors
    #[allow(dead_code)]
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
                    detector_type: detector.detector_type(),
                }
            })
            .collect()
    }

    /// Get the number of registered detectors
    #[allow(dead_code)]
    pub fn count(&self) -> usize {
        self.detectors.len()
    }

    /// Get the number of enabled detectors
    #[allow(dead_code)]
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
#[allow(dead_code)]
pub struct DetectorInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub default_severity: tower_lsp::lsp_types::DiagnosticSeverity,
    pub detector_type: DetectorType,
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

    /// Add a syn-based detector to the registry
    pub fn with_syn_detector<D: SynDetector + 'static>(mut self, detector: D) -> Self {
        self.registry.register_syn(detector);
        self
    }

    /// Add a clippy-style detector to the registry
    pub fn with_clippy_detector<D: ClippyDetector + 'static>(mut self, detector: D) -> Self {
        self.registry.register_clippy_detector(detector);
        self
    }

    /// Configure a detector
    #[allow(dead_code)]
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

/// Statistics about the detector system
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct DetectorStats {
    pub total_detectors: usize,
    pub enabled_detectors: usize,
    pub syn_detectors: usize,
    pub clippy_detectors: usize,
}
