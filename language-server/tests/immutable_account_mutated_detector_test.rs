use language_server::core::detectors::{
    detector::Detector, immutable_account_mutated_detector::ImmutableAccountMutatedDetector,
};
use tower_lsp::lsp_types::DiagnosticSeverity;

#[test]
fn test_detector_metadata() {
    let detector = ImmutableAccountMutatedDetector::default();

    assert_eq!(detector.id(), "IMMUTABLE_ACCOUNT_MUTATED");
    assert_eq!(detector.name(), "Immutable Account Mutation");
    assert_eq!(
        detector.description(),
        "Detects attempts to mutate accounts that are not marked as mutable with #[account(mut)]"
    );
    assert_eq!(detector.default_severity(), DiagnosticSeverity::ERROR);
}

#[test]
fn test_detects_immutable_account_mutation() {
    let mut detector = ImmutableAccountMutatedDetector::default();

    let code_with_immutable_mutation = r#"
        use anchor_lang::prelude::*;

        #[derive(Accounts)]
        pub struct VulnerableAccounts<'info> {
            pub vault: Account<'info, Vault>,
            #[account(mut)]
            pub user_account: Account<'info, UserAccount>,
        }

        #[program]
        pub mod vulnerable_program {
            use super::*;

            pub fn dangerous_function(ctx: Context<VulnerableAccounts>) -> Result<()> {
                // This should be flagged - vault is not marked as mutable
                ctx.accounts.vault.balance = 100;

                // This should be ok - user_account is marked as mutable
                ctx.accounts.user_account.balance = 200;

                Ok(())
            }
        }

        #[account]
        pub struct Vault {
            pub balance: u64,
        }

        #[account]
        pub struct UserAccount {
            pub balance: u64,
        }
    "#;

    let diagnostics = detector.analyze(code_with_immutable_mutation);
    assert_eq!(diagnostics.len(), 1);

    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::ERROR));
    assert!(diagnostic.message.contains("vault"));
    assert!(diagnostic.message.contains("#[account(mut)]"));
}

#[test]
fn test_detects_method_call_mutation() {
    let mut detector = ImmutableAccountMutatedDetector::default();

    let code_with_method_mutation = r#"
        use anchor_lang::prelude::*;

        #[derive(Accounts)]
        pub struct TestAccounts<'info> {
            pub readonly_account: AccountInfo<'info>,
            #[account(mut)]
            pub mutable_account: AccountInfo<'info>,
        }

        #[program]
        pub mod test_program {
            use super::*;

            pub fn test_function(ctx: Context<TestAccounts>) -> Result<()> {
                // This should be flagged - readonly_account is not marked as mutable
                ctx.accounts.readonly_account.set_lamports(0);

                // This should be ok - mutable_account is marked as mutable
                ctx.accounts.mutable_account.set_lamports(100);

                Ok(())
            }
        }
    "#;

    let diagnostics = detector.analyze(code_with_method_mutation);
    println!("Diagnostics found: {}", diagnostics.len());
    for diagnostic in &diagnostics {
        println!("Diagnostic: {}", diagnostic.message);
    }
    assert_eq!(diagnostics.len(), 1);

    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::ERROR));
    assert!(diagnostic.message.contains("readonly_account"));
}

#[test]
fn test_no_detection_for_mutable_accounts() {
    let mut detector = ImmutableAccountMutatedDetector::default();

    let safe_code = r#"
        use anchor_lang::prelude::*;

        #[derive(Accounts)]
        pub struct SafeAccounts<'info> {
            #[account(mut)]
            pub vault: Account<'info, Vault>,
            #[account(mut)]
            pub user_account: Account<'info, UserAccount>,
        }

        #[program]
        pub mod safe_program {
            use super::*;

            pub fn safe_function(ctx: Context<SafeAccounts>) -> Result<()> {
                // All mutations are safe since accounts are marked as mutable
                ctx.accounts.vault.balance = 100;
                ctx.accounts.user_account.balance = 200;

                Ok(())
            }
        }

        #[account]
        pub struct Vault {
            pub balance: u64,
        }

        #[account]
        pub struct UserAccount {
            pub balance: u64,
        }
    "#;

    let diagnostics = detector.analyze(safe_code);
    assert_eq!(diagnostics.len(), 0);
}

#[test]
fn test_ignores_non_account_fields() {
    let mut detector = ImmutableAccountMutatedDetector::default();

    let code_with_non_accounts = r#"
        use anchor_lang::prelude::*;

        #[derive(Accounts)]
        pub struct MixedAccounts<'info> {
            pub vault: Account<'info, Vault>,
            pub authority: Signer<'info>,
            pub system_program: Program<'info, System>,
        }

        #[program]
        pub mod mixed_program {
            use super::*;

            pub fn mixed_function(ctx: Context<MixedAccounts>) -> Result<()> {
                // This should be flagged - vault is not marked as mutable
                ctx.accounts.vault.balance = 100;

                // These should not be flagged since Signer and Program are not mutable account types
                // (Note: these lines would actually cause compilation errors, but we're testing detection)

                Ok(())
            }
        }

        #[account]
        pub struct Vault {
            pub balance: u64,
        }
    "#;

    let diagnostics = detector.analyze(code_with_non_accounts);
    assert_eq!(diagnostics.len(), 1); // Only the vault mutation should be detected
}

#[test]
fn test_ignores_non_accounts_structs() {
    let mut detector = ImmutableAccountMutatedDetector::default();

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

        #[program]
        pub mod test_program {
            use super::*;

            pub fn test_function(ctx: Context<VulnerableAccounts>) -> Result<()> {
                ctx.accounts.vault.balance = 100;
                Ok(())
            }
        }
    "#;

    let diagnostics = detector.analyze(code_with_mixed_structs);
    assert_eq!(diagnostics.len(), 1); // Only the Accounts struct should be analyzed
}

#[test]
fn test_complex_anchor_program() {
    let mut detector = ImmutableAccountMutatedDetector::default();

    let complex_program = r#"
        use anchor_lang::prelude::*;
        use anchor_spl::token::{Token, TokenAccount};

        #[program]
        pub mod complex_program {
            use super::*;

            pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
                // This should be flagged - vault is not marked as mutable
                ctx.accounts.vault.total_deposits = 0;
                Ok(())
            }

            pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
                // This should be ok - vault is marked as mutable in Deposit context
                ctx.accounts.vault.total_deposits += amount;
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
            pub user: Signer<'info>,
            pub token_program: Program<'info, Token>,
        }

        #[account]
        pub struct Vault {
            pub authority: Pubkey,
            pub total_deposits: u64,
        }
    "#;

    let diagnostics = detector.analyze(complex_program);
    assert_eq!(diagnostics.len(), 1); // Should detect the mutation in initialize function
}

#[test]
fn test_invalid_syntax_handling() {
    let mut detector = ImmutableAccountMutatedDetector::default();

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
