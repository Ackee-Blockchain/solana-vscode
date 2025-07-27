use std::path::PathBuf;
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};
use clippy_utils;

/// Base trait for all security detectors in Anchor programs
/// Contains common functionality shared by both syn and clippy detectors
pub trait Detector: Send + Sync {
    /// Unique identifier for this detector
    fn id(&self) -> &'static str;

    /// Human-readable name for this detector
    fn name(&self) -> &'static str;

    /// Description of what this detector checks for
    fn description(&self) -> &'static str;

    /// Message for detection
    fn message(&self) -> &'static str;

    /// Severity level for diagnostics produced by this detector
    fn default_severity(&self) -> DiagnosticSeverity;
}

/// Trait specifically for syn-based detectors (fast, syntax-only analysis)
pub trait SynDetector: Detector {
    /// Analyze the given content and return any security issues found
    fn analyze(&mut self, content: &str, file_path: Option<&PathBuf>) -> Vec<Diagnostic>;

    /// Returns the detector type (always Syn for SynDetector)
    fn detector_type(&self) -> DetectorType {
        DetectorType::Syn
    }
}

/// Trait for clippy-style detectors that need compilation context
pub trait ClippyDetector: Detector {
    /// Perform clippy-style analysis (slower, comprehensive)
    fn analyze_with_context(&mut self, analysis_context: &ClippyAnalysisContext)
    -> Vec<Diagnostic>;

    /// Returns the detector type (always Clippy for ClippyDetector)
    fn detector_type(&self) -> DetectorType {
        DetectorType::Clippy
    }
}

/// Wrapper enum to store different types of detectors
pub enum DetectorWrapper {
    Syn(Box<dyn SynDetector>),
    Clippy(Box<dyn ClippyDetector>),
}

impl DetectorWrapper {
    /// Create a new syn detector wrapper
    pub fn new_syn<D: SynDetector + 'static>(detector: D) -> Self {
        Self::Syn(Box::new(detector))
    }

    /// Create a new clippy detector wrapper
    pub fn new_clippy<D: ClippyDetector + 'static>(detector: D) -> Self {
        Self::Clippy(Box::new(detector))
    }

    /// Get reference to the base Detector trait
    fn as_detector(&self) -> &dyn Detector {
        match self {
            Self::Syn(detector) => detector.as_ref(),
            Self::Clippy(detector) => detector.as_ref(),
        }
    }

    /// Get the detector ID
    pub fn id(&self) -> &'static str {
        self.as_detector().id()
    }

    /// Get the detector name
    pub fn name(&self) -> &'static str {
        self.as_detector().name()
    }

    /// Get the detector description
    pub fn description(&self) -> &'static str {
        self.as_detector().description()
    }

    /// Get the detector message
    pub fn message(&self) -> &'static str {
        self.as_detector().message()
    }

    /// Get the default severity
    pub fn default_severity(&self) -> DiagnosticSeverity {
        self.as_detector().default_severity()
    }

    /// Get the detector type
    pub fn detector_type(&self) -> DetectorType {
        match self {
            Self::Syn(detector) => detector.detector_type(),
            Self::Clippy(detector) => detector.detector_type(),
        }
    }

    /// Analyze content with syn detector (returns empty Vec for clippy detectors)
    pub fn analyze_syn(&mut self, content: &str, file_path: Option<&PathBuf>) -> Vec<Diagnostic> {
        match self {
            Self::Syn(detector) => detector.analyze(content, file_path),
            Self::Clippy(_) => Vec::new(), // Clippy detectors don't do immediate analysis
        }
    }

    /// Analyze content with clippy detector (returns empty Vec for syn detectors)
    pub fn analyze_clippy(&mut self, context: &ClippyAnalysisContext) -> Vec<Diagnostic> {
        match self {
            Self::Syn(_) => Vec::new(), // Syn detectors don't use compilation context
            Self::Clippy(detector) => detector.analyze_with_context(context),
        }
    }

    /// Check if this is a syn detector
    pub fn is_syn(&self) -> bool {
        matches!(self, Self::Syn(_))
    }

    /// Check if this is a clippy detector
    pub fn is_clippy(&self) -> bool {
        matches!(self, Self::Clippy(_))
    }
}

/// Analysis context for clippy-style detectors with essential compilation information
#[derive(Debug)]
pub struct ClippyAnalysisContext {
    pub file_path: PathBuf,
    pub source_code: String,
    /// Whether the compilation was successful
    pub compilation_successful: bool,
}

impl Default for ClippyAnalysisContext {
    fn default() -> Self {
        Self {
            file_path: PathBuf::new(),
            source_code: String::new(),
            compilation_successful: false,
        }
    }
}

/// Type of detector for optimization and scheduling
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectorType {
    /// Fast syn-based analysis
    Syn,
    /// Slower clippy-style analysis with type information
    Clippy,
}
