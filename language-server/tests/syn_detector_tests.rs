// Integration tests for syn detectors
// This file allows cargo test to discover and run all syn detector tests

mod syn_detectors {
    mod immutable_account_mutated_detector_test;
    mod instruction_attribute_invalid_test;
    mod instruction_attribute_unused_test;
    mod manual_lamports_zeroing_test;
    mod missing_check_comment_test;
    mod missing_initspace_detector_test;
    mod missing_signer_detector_test;
    mod sysvar_account_detector_test;
    mod unsafe_math_detector_test;
}
