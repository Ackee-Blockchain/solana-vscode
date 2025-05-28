use language_server::core::detectors::{detector::Detector, missing_signer::MissingSignerDetector};
use tower_lsp::lsp_types::DiagnosticSeverity;

#[test]
fn test_detector_metadata() {
    let detector = MissingSignerDetector::new();

    assert_eq!(detector.id(), "MISSING_SIGNER");
    assert_eq!(detector.name(), "Missing Signer Check");
    assert_eq!(detector.description(), "Detects Anchor accounts structs that have no signer accounts, which could allow unauthorized access");
    assert_eq!(detector.default_severity(), DiagnosticSeverity::WARNING);
}

#[test]
fn test_should_run_with_accounts_derive() {
    let detector = MissingSignerDetector::new();

    let anchor_with_accounts = r#"
        use anchor_lang::prelude::*;

        #[derive(Accounts)]
        pub struct MyAccounts<'info> {
            pub account: Account<'info, MyAccount>,
        }
    "#;
    assert!(detector.should_run(anchor_with_accounts));
}

#[test]
fn test_should_not_run_without_anchor() {
    let detector = MissingSignerDetector::new();

    let no_anchor = r#"
        #[derive(Accounts)]
        pub struct MyAccounts {
            pub account: String,
        }
    "#;
    assert!(!detector.should_run(no_anchor));
}

#[test]
fn test_should_not_run_without_accounts_derive() {
    let detector = MissingSignerDetector::new();

    let anchor_no_accounts = r#"
        use anchor_lang::prelude::*;

        pub struct MyStruct {
            pub field: u64,
        }
    "#;
    assert!(!detector.should_run(anchor_no_accounts));
}

#[test]
fn test_detects_missing_signer() {
    let mut detector = MissingSignerDetector::new();

    let code_without_signer = r#"
        use anchor_lang::prelude::*;

        #[derive(Accounts)]
        pub struct VulnerableAccounts<'info> {
            #[account(mut)]
            pub vault: Account<'info, Vault>,
            pub token_account: Account<'info, TokenAccount>,
        }

        #[account]
        pub struct Vault {
            pub balance: u64,
        }
    "#;

    let diagnostics = detector.analyze(code_without_signer);
    assert_eq!(diagnostics.len(), 1);

    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::WARNING));
    assert!(diagnostic.message.contains("Accounts struct has no signer"));
    assert!(diagnostic.message.contains("Signer<'info>"));
}

#[test]
fn test_no_detection_with_signer() {
    let mut detector = MissingSignerDetector::new();

    let code_with_signer = r#"
        use anchor_lang::prelude::*;

        #[derive(Accounts)]
        pub struct SecureAccounts<'info> {
            #[account(mut)]
            pub vault: Account<'info, Vault>,
            pub user: Signer<'info>,
            pub token_account: Account<'info, TokenAccount>,
        }

        #[account]
        pub struct Vault {
            pub balance: u64,
        }
    "#;

    let diagnostics = detector.analyze(code_with_signer);
    assert_eq!(diagnostics.len(), 0);
}

#[test]
fn test_detects_multiple_accounts_structs() {
    let mut detector = MissingSignerDetector::new();

    let code_with_multiple_structs = r#"
        use anchor_lang::prelude::*;

        #[derive(Accounts)]
        pub struct VulnerableAccounts1<'info> {
            pub vault: Account<'info, Vault>,
        }

        #[derive(Accounts)]
        pub struct SecureAccounts<'info> {
            pub vault: Account<'info, Vault>,
            pub authority: Signer<'info>,
        }

        #[derive(Accounts)]
        pub struct VulnerableAccounts2<'info> {
            pub token_account: Account<'info, TokenAccount>,
        }

        #[account]
        pub struct Vault {
            pub balance: u64,
        }
    "#;

    let diagnostics = detector.analyze(code_with_multiple_structs);
    assert_eq!(diagnostics.len(), 2); // Should detect 2 vulnerable structs

    for diagnostic in &diagnostics {
        assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::WARNING));
        assert!(diagnostic.message.contains("Accounts struct has no signer"));
    }
}

#[test]
fn test_ignores_non_accounts_structs() {
    let mut detector = MissingSignerDetector::new();

    let code_with_mixed_structs = r#"
        use anchor_lang::prelude::*;

        // This should be ignored (no #[derive(Accounts)])
        pub struct RegularStruct {
            pub field: u64,
        }

        #[derive(Accounts)]
        pub struct VulnerableAccounts<'info> {
            pub vault: Account<'info, Vault>,
        }

        // This should also be ignored
        #[account]
        pub struct Vault {
            pub balance: u64,
        }
    "#;

    let diagnostics = detector.analyze(code_with_mixed_structs);
    assert_eq!(diagnostics.len(), 1); // Only the Accounts struct should be flagged
}

#[test]
fn test_different_signer_patterns() {
    let mut detector = MissingSignerDetector::new();

    let code_with_various_signers = r#"
        use anchor_lang::prelude::*;

        #[derive(Accounts)]
        pub struct WithSigner1<'info> {
            pub authority: Signer<'info>,
            pub vault: Account<'info, Vault>,
        }

        #[derive(Accounts)]
        pub struct WithSigner2<'info> {
            pub vault: Account<'info, Vault>,
            pub user: Signer<'info>,
        }

        #[derive(Accounts)]
        pub struct WithMultipleSigners<'info> {
            pub admin: Signer<'info>,
            pub user: Signer<'info>,
            pub vault: Account<'info, Vault>,
        }
    "#;

    let diagnostics = detector.analyze(code_with_various_signers);
    assert_eq!(diagnostics.len(), 0); // All structs have signers
}

#[test]
fn test_real_world_anchor_patterns() {
    let mut detector = MissingSignerDetector::new();

    let vulnerable_transfer = r#"
        use anchor_lang::prelude::*;
        use anchor_spl::token::{Token, TokenAccount};

        #[derive(Accounts)]
        pub struct VulnerableTransfer<'info> {
            #[account(mut)]
            pub from: Account<'info, TokenAccount>,
            #[account(mut)]
            pub to: Account<'info, TokenAccount>,
            pub token_program: Program<'info, Token>,
        }
    "#;

    let diagnostics = detector.analyze(vulnerable_transfer);
    assert_eq!(diagnostics.len(), 1);

    let secure_transfer = r#"
        use anchor_lang::prelude::*;
        use anchor_spl::token::{Token, TokenAccount};

        #[derive(Accounts)]
        pub struct SecureTransfer<'info> {
            #[account(mut)]
            pub from: Account<'info, TokenAccount>,
            #[account(mut)]
            pub to: Account<'info, TokenAccount>,
            pub authority: Signer<'info>,
            pub token_program: Program<'info, Token>,
        }
    "#;

    let diagnostics = detector.analyze(secure_transfer);
    assert_eq!(diagnostics.len(), 0);
}

#[test]
fn test_invalid_syntax_handling() {
    let mut detector = MissingSignerDetector::new();

    let invalid_code = r#"
        use anchor_lang::prelude::*;

        #[derive(Accounts)]
        pub struct BrokenStruct<'info> {
            pub field: Account<'info,
        }
    "#;

    // Should handle invalid syntax gracefully
    let diagnostics = detector.analyze(invalid_code);
    assert_eq!(diagnostics.len(), 0);
}

#[test]
fn test_detector_state_isolation() {
    let mut detector1 = MissingSignerDetector::new();
    let mut detector2 = MissingSignerDetector::new();

    let code = r#"
        use anchor_lang::prelude::*;

        #[derive(Accounts)]
        pub struct TestAccounts<'info> {
            pub vault: Account<'info, Vault>,
        }
    "#;

    let diagnostics1 = detector1.analyze(code);
    let diagnostics2 = detector2.analyze(code);

    // Each detector instance should produce the same results
    assert_eq!(diagnostics1.len(), diagnostics2.len());
    assert_eq!(diagnostics1.len(), 1);
}

#[test]
fn test_complex_anchor_program() {
    let mut detector = MissingSignerDetector::new();

    let complex_program = r#"
        use anchor_lang::prelude::*;
        use anchor_spl::token::{Token, TokenAccount, Mint};

        #[program]
        pub mod token_vault {
            use super::*;

            pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
                Ok(())
            }

            pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
                Ok(())
            }
        }

        #[derive(Accounts)]
        pub struct Initialize<'info> {
            #[account(init, payer = authority, space = 8 + 32)]
            pub vault: Account<'info, Vault>,
            #[account(mut)]
            pub authority: Signer<'info>,
            pub system_program: Program<'info, System>,
        }

        #[derive(Accounts)]
        pub struct Deposit<'info> {
            #[account(mut)]
            pub vault: Account<'info, Vault>,
            #[account(mut)]
            pub user_token_account: Account<'info, TokenAccount>,
            #[account(mut)]
            pub vault_token_account: Account<'info, TokenAccount>,
            pub user: Signer<'info>,
            pub token_program: Program<'info, Token>,
        }

        // This one is vulnerable - no signer!
        #[derive(Accounts)]
        pub struct VulnerableWithdraw<'info> {
            #[account(mut)]
            pub vault: Account<'info, Vault>,
            #[account(mut)]
            pub user_token_account: Account<'info, TokenAccount>,
            #[account(mut)]
            pub vault_token_account: Account<'info, TokenAccount>,
            pub token_program: Program<'info, Token>,
        }

        #[account]
        pub struct Vault {
            pub authority: Pubkey,
            pub total_deposits: u64,
        }
    "#;

    let diagnostics = detector.analyze(complex_program);
    assert_eq!(diagnostics.len(), 1); // Only VulnerableWithdraw should be flagged

    let diagnostic = &diagnostics[0];
    assert!(diagnostic.message.contains("Accounts struct has no signer"));
}
