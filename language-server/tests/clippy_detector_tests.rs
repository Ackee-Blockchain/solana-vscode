// Integration tests for clippy detectors
// This file allows cargo test to discover and run all clippy detector tests
#![feature(rustc_private)]
mod clippy_detectors {
    mod unsafe_math_detector_test;
}
