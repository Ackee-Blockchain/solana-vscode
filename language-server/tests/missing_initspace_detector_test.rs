use language_server::core::detectors::{
    detector::Detector, missing_initspace_detector::MissingInitspaceDetector,
};
use tower_lsp::lsp_types::DiagnosticSeverity;

#[test]
fn test_detector_metadata() {
    let detector = MissingInitspaceDetector::default();

    assert_eq!(detector.id(), "MISSING_INITSPACE");
    assert_eq!(detector.name(), "Missing InitSpace macro");
    assert_eq!(
        detector.description(),
        "Detects Anchor accounts structs that don't use the #[derive(InitSpace)] macro"
    );
    assert_eq!(detector.default_severity(), DiagnosticSeverity::WARNING);
}

#[test]
fn test_detects_missing_initspace() {
    let mut detector = MissingInitspaceDetector::default();

    let code_without_initspace = r#"
        use anchor_lang::prelude::*;

        #[account]
        #[derive(Default)]
        pub struct VulnerableAccount {
            pub balance: u64,
            pub owner: Pubkey,
        }
    "#;

    let diagnostics = detector.analyze(code_without_initspace, None);
    assert_eq!(diagnostics.len(), 1);

    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::WARNING));
    assert!(diagnostic.message.contains("InitSpace"));
}

#[test]
fn test_no_detection_with_initspace() {
    let mut detector = MissingInitspaceDetector::default();

    let code_with_initspace = r#"
        use anchor_lang::prelude::*;

        #[account]
        #[derive(Default, InitSpace)]
        pub struct SecureAccount {
            pub balance: u64,
            pub owner: Pubkey,
        }
    "#;

    let diagnostics = detector.analyze(code_with_initspace, None);
    assert_eq!(diagnostics.len(), 0);
}

#[test]
fn test_detects_multiple_account_structs() {
    let mut detector = MissingInitspaceDetector::default();

    let code_with_multiple_structs = r#"
        use anchor_lang::prelude::*;

        #[account]
        #[derive(Default)]
        pub struct VulnerableAccount1 {
            pub balance: u64,
        }

        #[account]
        #[derive(Default, InitSpace)]
        pub struct SecureAccount {
            pub balance: u64,
            pub owner: Pubkey,
        }

        #[account]
        #[derive(Default)]
        pub struct VulnerableAccount2 {
            pub token_amount: u64,
        }
    "#;

    let diagnostics = detector.analyze(code_with_multiple_structs, None);
    assert_eq!(diagnostics.len(), 2); // Should detect 2 vulnerable structs

    for diagnostic in &diagnostics {
        assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::WARNING));
        assert!(diagnostic.message.contains("InitSpace"));
    }
}

#[test]
fn test_ignores_non_account_structs() {
    let mut detector = MissingInitspaceDetector::default();

    let code_with_mixed_structs = r#"
        use anchor_lang::prelude::*;

        // This should be ignored (no #[account])
        pub struct RegularStruct {
            pub field: u64,
        }

        #[account]
        #[derive(Default)]
        pub struct VulnerableAccount {
            pub balance: u64,
        }

        // This should also be ignored
        #[derive(Accounts)]
        pub struct Instructions<'info> {
            pub vault: Account<'info, VulnerableAccount>,
        }
    "#;

    let diagnostics = detector.analyze(code_with_mixed_structs, None);
    assert_eq!(diagnostics.len(), 1); // Only the account struct should be flagged
}

#[test]
fn test_different_initspace_patterns() {
    let mut detector = MissingInitspaceDetector::default();

    let code_with_various_patterns = r#"
        use anchor_lang::prelude::*;

        #[account]
        #[derive(InitSpace, Default)]
        pub struct Account1 {
            pub balance: u64,
        }

        #[account]
        #[derive(Default, InitSpace)]
        pub struct Account2 {
            pub balance: u64,
        }

        #[account]
        #[derive(Clone, Default, InitSpace, Debug)]
        pub struct Account3 {
            pub balance: u64,
        }
    "#;

    let diagnostics = detector.analyze(code_with_various_patterns, None);
    assert_eq!(diagnostics.len(), 0); // All structs have InitSpace
}

#[test]
fn test_real_world_anchor_patterns() {
    let mut detector = MissingInitspaceDetector::default();

    let vulnerable_account = r#"
        use anchor_lang::prelude::*;

        #[account]
        #[derive(Default)]
        pub struct VulnerableAccount {
            pub balance: u64,
            pub owner: Pubkey,
            pub created_at: i64,
            pub data: Vec<u8>,
        }
    "#;

    let diagnostics = detector.analyze(vulnerable_account, None);
    assert_eq!(diagnostics.len(), 1);

    let secure_account = r#"
        use anchor_lang::prelude::*;

        #[account]
        #[derive(Default, InitSpace)]
        pub struct SecureAccount {
            pub balance: u64,
            pub owner: Pubkey,
            pub created_at: i64,
            #[max_len(100)]
            pub data: Vec<u8>,
        }
    "#;

    let diagnostics = detector.analyze(secure_account, None);
    assert_eq!(diagnostics.len(), 0);
}

#[test]
fn test_invalid_syntax_handling() {
    let mut detector = MissingInitspaceDetector::default();

    let invalid_code = r#"
        use anchor_lang::prelude::*;

        #[account]
        #[derive(Default]
        pub struct BrokenStruct {
            pub field: u64,
        }
    "#;

    // Should handle invalid syntax gracefully
    let diagnostics = detector.analyze(invalid_code, None);
    assert_eq!(diagnostics.len(), 0);
}

#[test]
fn test_detector_state_isolation() {
    let mut detector1 = MissingInitspaceDetector::default();
    let mut detector2 = MissingInitspaceDetector::default();

    let code = r#"
        use anchor_lang::prelude::*;

        #[account]
        #[derive(Default)]
        pub struct TestAccount {
            pub balance: u64,
        }
    "#;

    let diagnostics1 = detector1.analyze(code, None);
    let diagnostics2 = detector2.analyze(code, None);

    // Each detector instance should produce the same results
    assert_eq!(diagnostics1.len(), diagnostics2.len());
    assert_eq!(diagnostics1.len(), 1);
}
