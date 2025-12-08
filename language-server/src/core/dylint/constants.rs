/// The specific nightly Rust version required by this extension
/// All dylint detectors must be compatible with this version
pub const REQUIRED_NIGHTLY_VERSION: &str = "nightly-2025-09-18";

/// Components required for building dylint detectors
pub const REQUIRED_COMPONENTS: &[&str] = &["llvm-tools-preview", "rustc-dev"];

