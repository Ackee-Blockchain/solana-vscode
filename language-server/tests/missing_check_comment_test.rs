#[cfg(test)]
mod tests {
    use language_server::core::detectors::detector::Detector;
    use language_server::core::detectors::missing_check_comment::MissingCheckCommentDetector;
    use tower_lsp::lsp_types::DiagnosticSeverity;

    #[test]
    fn test_missing_check_comment_accountinfo() {
        let mut detector = MissingCheckCommentDetector::default();

        let code = r#"
#[derive(Accounts)]
pub struct InstructionAccounts<'info> {
    pub unchecked_account: AccountInfo<'info>,
}
"#;

        let diagnostics = detector.analyze(code, None);

        // Should report error for missing CHECK comment
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, Some(DiagnosticSeverity::ERROR));
        assert!(diagnostics[0].message.contains("Missing /// CHECK:"));
        assert!(diagnostics[0].message.contains("unchecked_account"));
        assert!(diagnostics[0].message.contains("AccountInfo"));
    }

    #[test]
    fn test_missing_check_comment_unchecked_account() {
        let mut detector = MissingCheckCommentDetector::default();

        let code = r#"
#[derive(Accounts)]
pub struct InstructionAccounts<'info> {
    pub account: UncheckedAccount<'info>,
}
"#;

        let diagnostics = detector.analyze(code, None);

        // Should report error for missing CHECK comment
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, Some(DiagnosticSeverity::ERROR));
        assert!(diagnostics[0].message.contains("Missing /// CHECK:"));
        assert!(diagnostics[0].message.contains("account"));
        assert!(diagnostics[0].message.contains("UncheckedAccount"));
    }

    #[test]
    fn test_valid_check_comment_accountinfo() {
        let mut detector = MissingCheckCommentDetector::default();

        let code = r#"
#[derive(Accounts)]
pub struct InstructionAccounts<'info> {
    /// CHECK: AccountInfo is an unchecked account
    pub unchecked_account: AccountInfo<'info>,
}
"#;

        let diagnostics = detector.analyze(code, None);

        // Should be no diagnostics for valid CHECK comment
        assert_eq!(diagnostics.len(), 0);
    }

    #[test]
    fn test_valid_check_comment_unchecked_account() {
        let mut detector = MissingCheckCommentDetector::default();

        let code = r#"
#[derive(Accounts)]
pub struct InstructionAccounts<'info> {
    /// CHECK: No checks are performed
    pub account: UncheckedAccount<'info>,
}
"#;

        let diagnostics = detector.analyze(code, None);

        // Should be no diagnostics for valid CHECK comment
        assert_eq!(diagnostics.len(), 0);
    }

    #[test]
    fn test_multiple_unchecked_accounts() {
        let mut detector = MissingCheckCommentDetector::default();

        let code = r#"
#[derive(Accounts)]
pub struct InstructionAccounts<'info> {
    /// CHECK: This is properly documented
    pub valid_account: AccountInfo<'info>,
    
    pub missing_check1: AccountInfo<'info>,
    pub missing_check2: UncheckedAccount<'info>,
    
    /// CHECK: Another valid one
    pub valid_unchecked: UncheckedAccount<'info>,
}
"#;

        let diagnostics = detector.analyze(code, None);

        // Should report errors for the two missing CHECK comments
        assert_eq!(diagnostics.len(), 2);

        // Check first error
        assert_eq!(diagnostics[0].severity, Some(DiagnosticSeverity::ERROR));
        assert!(diagnostics[0].message.contains("missing_check1"));

        // Check second error
        assert_eq!(diagnostics[1].severity, Some(DiagnosticSeverity::ERROR));
        assert!(diagnostics[1].message.contains("missing_check2"));
    }

    #[test]
    fn test_regular_doc_comment_not_check() {
        let mut detector = MissingCheckCommentDetector::default();

        let code = r#"
#[derive(Accounts)]
pub struct InstructionAccounts<'info> {
    /// This is just a regular doc comment
    pub unchecked_account: AccountInfo<'info>,
}
"#;

        let diagnostics = detector.analyze(code, None);

        // Should report error because it's not a CHECK comment
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, Some(DiagnosticSeverity::ERROR));
        assert!(diagnostics[0].message.contains("Missing /// CHECK:"));
    }

    #[test]
    fn test_other_account_types_ignored() {
        let mut detector = MissingCheckCommentDetector::default();

        let code = r#"
#[derive(Accounts)]
pub struct InstructionAccounts<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    
    pub account: Account<'info, MyAccount>,
    
    pub program: Program<'info, System>,
}
"#;

        let diagnostics = detector.analyze(code, None);

        // Should be no diagnostics for other account types
        assert_eq!(diagnostics.len(), 0);
    }

    #[test]
    fn test_non_accounts_struct_ignored() {
        let mut detector = MissingCheckCommentDetector::default();

        let code = r#"
pub struct SomeOtherStruct<'info> {
    pub unchecked_account: AccountInfo<'info>,
}
"#;

        let diagnostics = detector.analyze(code, None);

        // Should be no diagnostics for non-Accounts structs
        assert_eq!(diagnostics.len(), 0);
    }

    #[test]
    fn test_check_comment_with_extra_text() {
        let mut detector = MissingCheckCommentDetector::default();

        let code = r#"
#[derive(Accounts)]
pub struct InstructionAccounts<'info> {
    /// CHECK: This account is used for cross-program invocation and is validated by the target program
    pub unchecked_account: AccountInfo<'info>,
}
"#;

        let diagnostics = detector.analyze(code, None);

        // Should be no diagnostics for detailed CHECK comment
        assert_eq!(diagnostics.len(), 0);
    }

    #[test]
    fn test_multiline_check_comment() {
        let mut detector = MissingCheckCommentDetector::default();

        let code = r#"
#[derive(Accounts)]
pub struct InstructionAccounts<'info> {
    /// CHECK: This account is safe because:
    /// - It's only used for reading data
    /// - The program validates it internally
    pub unchecked_account: AccountInfo<'info>,
}
"#;

        let diagnostics = detector.analyze(code, None);

        // Should be no diagnostics for multiline CHECK comment
        assert_eq!(diagnostics.len(), 0);
    }
}
