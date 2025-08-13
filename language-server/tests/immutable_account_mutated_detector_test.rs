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

    let diagnostics = detector.analyze(code_with_immutable_mutation, None);
    assert_eq!(diagnostics.len(), 2); // Expect two diagnostics: one at mutation site, one at field definition

    // Check the mutation site diagnostic
    let mutation_diagnostic = diagnostics
        .iter()
        .find(|d| d.message.contains("Attempting to mutate"))
        .expect("Should have mutation diagnostic");
    assert_eq!(
        mutation_diagnostic.severity,
        Some(DiagnosticSeverity::ERROR)
    );
    assert!(mutation_diagnostic.message.contains("vault"));
    assert!(mutation_diagnostic.message.contains("#[account(mut)]"));
    assert!(mutation_diagnostic.related_information.is_some());

    // Check the field definition diagnostic
    let field_diagnostic = diagnostics
        .iter()
        .find(|d| d.message.contains("defined here"))
        .expect("Should have field definition diagnostic");
    assert_eq!(field_diagnostic.severity, Some(DiagnosticSeverity::ERROR));
    assert!(field_diagnostic.message.contains("vault"));
    assert!(field_diagnostic.message.contains("#[account(mut)]"));
    assert!(field_diagnostic.related_information.is_some());
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

    let diagnostics = detector.analyze(code_with_method_mutation, None);
    assert_eq!(diagnostics.len(), 2); // Expect two diagnostics: one at mutation site, one at field definition

    // Check the mutation site diagnostic
    let mutation_diagnostic = diagnostics
        .iter()
        .find(|d| d.message.contains("Attempting to mutate"))
        .expect("Should have mutation diagnostic");
    assert_eq!(
        mutation_diagnostic.severity,
        Some(DiagnosticSeverity::ERROR)
    );
    assert!(mutation_diagnostic.message.contains("readonly_account"));
    assert!(mutation_diagnostic.related_information.is_some());

    // Check the field definition diagnostic
    let field_diagnostic = diagnostics
        .iter()
        .find(|d| d.message.contains("defined here"))
        .expect("Should have field definition diagnostic");
    assert_eq!(field_diagnostic.severity, Some(DiagnosticSeverity::ERROR));
    assert!(field_diagnostic.message.contains("readonly_account"));
    assert!(field_diagnostic.related_information.is_some());
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

    let diagnostics = detector.analyze(safe_code, None);
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

    let diagnostics = detector.analyze(code_with_non_accounts, None);
    assert_eq!(diagnostics.len(), 2); // Expect two diagnostics: one at mutation site, one at field definition
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

    let diagnostics = detector.analyze(code_with_mixed_structs, None);
    assert_eq!(diagnostics.len(), 2); // Expect two diagnostics: one at mutation site, one at field definition
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
                // This should be ok - vault is initialized in the Initialize context
                ctx.accounts.vault.total_deposits = 0;
                Ok(())
            }

            pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
                // This should be flagged - vault is NOT marked as mutable in Deposit context
                ctx.accounts.vault.total_deposits = amount;
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

    let diagnostics = detector.analyze(complex_program, None);
    assert_eq!(diagnostics.len(), 2); // Expect two diagnostics: one at mutation site, one at field definition
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
    let diagnostics = detector.analyze(invalid_code, None);
    assert_eq!(diagnostics.len(), 0);
}

#[test]
fn test_detects_mutation_through_reference() {
    let mut detector = ImmutableAccountMutatedDetector::default();

    let code_with_ref_mutation = r#"
        use anchor_lang::prelude::*;

        #[program]
        pub mod test_program {
            use super::*;

            pub fn mutating_accounts_check(ctx: Context<MutatingAccountsCheck>) -> Result<()> {
                let counter = &mut ctx.accounts.mutating_account;
                counter.value += 1;
                Ok(())
            }
        }

        #[derive(Accounts)]
        pub struct MutatingAccountsCheck<'info> {
            pub mutating_account: Account<'info, Counter>,
            #[account(mut)]
            pub payer: Signer<'info>,
        }

        #[account]
        pub struct Counter {
            pub value: u64,
        }
    "#;

    let diagnostics = detector.analyze(code_with_ref_mutation, None);
    assert_eq!(diagnostics.len(), 2); // Expect two diagnostics: one at mutation site, one at field definition

    // Check the mutation site diagnostic
    let mutation_diagnostic = diagnostics
        .iter()
        .find(|d| d.message.contains("Attempting to mutate"))
        .expect("Should have mutation diagnostic");
    assert_eq!(
        mutation_diagnostic.severity,
        Some(DiagnosticSeverity::ERROR)
    );
    assert!(mutation_diagnostic.message.contains("mutating_account"));
    assert!(mutation_diagnostic.message.contains("#[account(mut)]"));
    assert!(mutation_diagnostic.related_information.is_some());

    // Check the field definition diagnostic
    let field_diagnostic = diagnostics
        .iter()
        .find(|d| d.message.contains("defined here"))
        .expect("Should have field definition diagnostic");
    assert_eq!(field_diagnostic.severity, Some(DiagnosticSeverity::ERROR));
    assert!(field_diagnostic.message.contains("mutating_account"));
    assert!(field_diagnostic.related_information.is_some());
}

/// Two-level dereference with a chained call to try_borrow_mut_lamports()
/// Expected: should be flagged (account not marked #[account(mut)] is attempting to modify lamports)
#[test]
fn test_detects_lamports_zeroing_through_chain() {
    let mut detector = ImmutableAccountMutatedDetector::default();

    let code = r#"
        use anchor_lang::prelude::*;

        #[derive(Accounts)]
        pub struct Case1<'info> {
            pub vault: AccountInfo<'info>,
            #[account(mut)]
            pub payer: Signer<'info>,
        }

        #[program]
        pub mod p1 {
            use super::*;
            pub fn f(ctx: Context<Case1>) -> Result<()> {
                **ctx.accounts.vault.try_borrow_mut_lamports()? = 0;
                Ok(())
            }
        }
    "#;

    let diagnostics = detector.analyze(code, None);
    assert_eq!(diagnostics.len(), 2); // mutation-site + field-definition
    let mutation = diagnostics.iter().find(|d| d.message.contains("Attempting to mutate")).expect("missing mutation diag");
    assert_eq!(mutation.severity, Some(DiagnosticSeverity::ERROR));
    assert!(mutation.message.contains("vault"));
    let defined = diagnostics.iter().find(|d| d.message.contains("defined here")).expect("missing field-definition diag");
    assert_eq!(defined.severity, Some(DiagnosticSeverity::ERROR));
    assert!(defined.message.contains("vault"));
}

/// Same as above but with extra parentheses and to_account_info() variant
/// Expected: should be flagged
#[test]
fn test_detects_lamports_zeroing_with_parentheses_and_to_account_info() {
    let mut detector = ImmutableAccountMutatedDetector::default();

    let code = r#"
        use anchor_lang::prelude::*;

        #[derive(Accounts)]
        pub struct Case1b<'info> {
            pub vault: Account<'info, Vault>,
            #[account(mut)]
            pub payer: Signer<'info>,
        }

        #[account]
        pub struct Vault { pub balance: u64 }

        #[program]
        pub mod p1b {
            use super::*;
            pub fn f(ctx: Context<Case1b>) -> Result<()> {
                **(ctx.accounts.vault.to_account_info()).try_borrow_mut_lamports()? = 0;
                Ok(())
            }
        }
    "#;

    let diagnostics = detector.analyze(code, None);
    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().any(|d| d.message.contains("Attempting to mutate") && d.message.contains("vault")));
    assert!(diagnostics.iter().any(|d| d.message.contains("defined here") && d.message.contains("vault")));
}

/// Directly replacing the entire account data structure (not just assigning to a field)
/// Expected: should be flagged (overwriting an account not marked #[account(mut)])
#[test]
fn test_detects_whole_struct_assignment() {
    let mut detector = ImmutableAccountMutatedDetector::default();

    let code = r#"
        use anchor_lang::prelude::*;

        #[derive(Accounts)]
        pub struct Case4<'info> {
            pub vault: Account<'info, Vault>,
            #[account(mut)]
            pub payer: Signer<'info>,
        }

        #[account]
        pub struct Vault { pub balance: u64 }

        #[program]
        pub mod p4 {
            use super::*;
            pub fn f(ctx: Context<Case4>) -> Result<()> {
                ctx.accounts.vault = Vault { balance: 999 };
                Ok(())
            }
        }
    "#;

    let diagnostics = detector.analyze(code, None);
    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().any(|d| d.message.contains("Attempting to mutate") && d.message.contains("vault")));
    assert!(diagnostics.iter().any(|d| d.message.contains("defined here") && d.message.contains("vault")));
}

/// Lamports modification on an UncheckedAccount variant
/// Expected: should be flagged
#[test]
fn test_detects_unchecked_account_lamports_mutation() {
    let mut detector = ImmutableAccountMutatedDetector::default();

    let code = r#"
        use anchor_lang::prelude::*;

        #[derive(Accounts)]
        pub struct Case8<'info> {
            pub victim: UncheckedAccount<'info>,
            #[account(mut)]
            pub payer: Signer<'info>,
        }

        #[program]
        pub mod p8 {
            use super::*;
            pub fn f(ctx: Context<Case8>) -> Result<()> {
                **ctx.accounts.victim.try_borrow_mut_lamports()? = 0;
                Ok(())
            }
        }
    "#;

    let diagnostics = detector.analyze(code, None);
    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().any(|d| d.message.contains("Attempting to mutate") && d.message.contains("victim")));
    assert!(diagnostics.iter().any(|d| d.message.contains("defined here") && d.message.contains("victim")));
}

/// Internal mutation caused by calling a method that takes &mut self (not direct field assignment)
/// Expected: should be flagged (calling a method that mutates an account not marked #[account(mut)])
#[test]
fn test_detects_mutation_via_method_with_mut_self() {
    let mut detector = ImmutableAccountMutatedDetector::default();

    let code = r#"
        use anchor_lang::prelude::*;

        #[account]
        pub struct Vault { pub balance: u64 }
        impl Vault {
            pub fn inc(&mut self) { self.balance += 1; }
        }

        #[derive(Accounts)]
        pub struct Case9<'info> {
            pub vault: Account<'info, Vault>,
            #[account(mut)]
            pub payer: Signer<'info>,
        }

        #[program]
        pub mod p9 {
            use super::*;
            pub fn f(ctx: Context<Case9>) -> Result<()> {
                ctx.accounts.vault.inc();
                Ok(())
            }
        }
    "#;

    let diagnostics = detector.analyze(code, None);
    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().any(|d| d.message.contains("Attempting to mutate") && d.message.contains("vault")));
    assert!(diagnostics.iter().any(|d| d.message.contains("defined here") && d.message.contains("vault")));
}

/// Negative test: Local shadow variable that is a normal struct, not an account
/// Expected: should not be flagged
#[test]
fn test_negative_local_shadow_struct_is_not_account() {
    let mut detector = ImmutableAccountMutatedDetector::default();

    let code = r#"
        use anchor_lang::prelude::*;
        #[account]
        pub struct Vault { pub balance: u64 }

        #[derive(Accounts)]
        pub struct Case10<'info> {
            pub dummy: AccountInfo<'info>,
        }

        #[program]
        pub mod p10 {
            use super::*;
            pub fn f(_ctx: Context<Case10>) -> Result<()> {
                let mut vault = Vault { balance: 0 };
                vault.balance = 1;
                Ok(())
            }
        }
    "#;

    let diagnostics = detector.analyze(code, None);
    assert_eq!(diagnostics.len(), 0);
}

/// Negative test: Sysvar, Program, and Signer accounts
/// Expected: should not be flagged
#[test]
fn test_negative_sysvar_program_signer_not_flagged() {
    let mut detector = ImmutableAccountMutatedDetector::default();

    let code = r#"
        use anchor_lang::prelude::*;

        #[derive(Accounts)]
        pub struct Case11<'info> {
            pub rent: Sysvar<'info, Rent>,
            pub system_program: Program<'info, System>,
            pub authority: Signer<'info>,
        }

        #[program]
        pub mod p11 {
            use super::*;
            pub fn f(ctx: Context<Case11>) -> Result<()> {
                let _ = &ctx.accounts.rent;
                let _ = &ctx.accounts.system_program;
                let _ = &ctx.accounts.authority;
                Ok(())
            }
        }
    "#;

    let diagnostics = detector.analyze(code, None);
    assert_eq!(diagnostics.len(), 0);
}
