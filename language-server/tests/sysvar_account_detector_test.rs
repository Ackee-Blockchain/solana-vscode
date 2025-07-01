use language_server::core::detectors::{
    detector::Detector, sysvar_account_detector::SysvarAccountDetector,
};
use tower_lsp::lsp_types::DiagnosticSeverity;

#[test]
fn test_detector_metadata() {
    let detector = SysvarAccountDetector::default();

    assert_eq!(detector.id(), "INEFFICIENT_SYSVAR_ACCOUNT");
    assert_eq!(detector.name(), "Inefficient Sysvar Account Usage");
    assert_eq!(
        detector.description(),
        "Detects usage of Sysvar<'info, Type> accounts and suggests using the more efficient get() method"
    );
    assert_eq!(detector.default_severity(), DiagnosticSeverity::WARNING);
}

#[test]
fn test_detects_clock_sysvar_account() {
    let mut detector = SysvarAccountDetector::default();

    let code_with_clock_sysvar = r#"
        use anchor_lang::prelude::*;

        #[derive(Accounts)]
        pub struct MyContext<'info> {
            pub clock: Sysvar<'info, Clock>,
            #[account(mut)]
            pub user: Account<'info, User>,
        }

        #[account]
        pub struct User {
            pub timestamp: i64,
        }
    "#;

    let diagnostics = detector.analyze(code_with_clock_sysvar, None);
    assert_eq!(diagnostics.len(), 1);

    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::WARNING));
    assert!(diagnostic.message.contains("Clock::get()?"));
    assert!(diagnostic.message.contains("more efficient"));
}

#[test]
fn test_detects_epoch_schedule_sysvar_account() {
    let mut detector = SysvarAccountDetector::default();

    let code_with_epoch_schedule_sysvar = r#"
        use anchor_lang::prelude::*;

        #[derive(Accounts)]
        pub struct MyContext<'info> {
            pub epoch_schedule: Sysvar<'info, EpochSchedule>,
            #[account(mut)]
            pub user: Account<'info, User>,
        }
    "#;

    let diagnostics = detector.analyze(code_with_epoch_schedule_sysvar, None);
    assert_eq!(diagnostics.len(), 1);

    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::WARNING));
    assert!(diagnostic.message.contains("EpochSchedule::get()?"));
}

#[test]
fn test_detects_rent_sysvar_account() {
    let mut detector = SysvarAccountDetector::default();

    let code_with_rent_sysvar = r#"
        use anchor_lang::prelude::*;

        #[derive(Accounts)]
        pub struct MyContext<'info> {
            pub rent: Sysvar<'info, Rent>,
            #[account(mut)]
            pub user: Account<'info, User>,
        }
    "#;

    let diagnostics = detector.analyze(code_with_rent_sysvar, None);
    assert_eq!(diagnostics.len(), 1);

    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::WARNING));
    assert!(diagnostic.message.contains("Rent::get()?"));
}

#[test]
fn test_detects_slot_hashes_sysvar_account() {
    let mut detector = SysvarAccountDetector::default();

    let code_with_slot_hashes_sysvar = r#"
        use anchor_lang::prelude::*;

        #[derive(Accounts)]
        pub struct MyContext<'info> {
            pub slot_hashes: Sysvar<'info, SlotHashes>,
            #[account(mut)]
            pub user: Account<'info, User>,
        }
    "#;

    let diagnostics = detector.analyze(code_with_slot_hashes_sysvar, None);
    assert_eq!(diagnostics.len(), 1);

    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::WARNING));
    assert!(diagnostic.message.contains("SlotHashes::get()?"));
}

#[test]
fn test_detects_multiple_sysvar_accounts() {
    let mut detector = SysvarAccountDetector::default();

    let code_with_multiple_sysvars = r#"
        use anchor_lang::prelude::*;

        #[derive(Accounts)]
        pub struct MyContext<'info> {
            pub clock: Sysvar<'info, Clock>,
            pub rent: Sysvar<'info, Rent>,
            pub epoch_schedule: Sysvar<'info, EpochSchedule>,
            pub slot_hashes: Sysvar<'info, SlotHashes>,
            pub stake_history: Sysvar<'info, StakeHistory>,
            pub instructions: Sysvar<'info, Instructions>,
            #[account(mut)]
            pub user: Account<'info, User>,
        }
    "#;

    let diagnostics = detector.analyze(code_with_multiple_sysvars, None);
    assert_eq!(diagnostics.len(), 6);

    // Check that all sysvars are detected (since all support get() via trait)
    let messages: Vec<String> = diagnostics.iter().map(|d| d.message.clone()).collect();
    assert!(messages.iter().any(|m| m.contains("Clock::get()?")));
    assert!(messages.iter().any(|m| m.contains("Rent::get()?")));
    assert!(messages.iter().any(|m| m.contains("EpochSchedule::get()?")));
    assert!(messages.iter().any(|m| m.contains("SlotHashes::get()?")));
    assert!(messages.iter().any(|m| m.contains("StakeHistory::get()?")));
    assert!(messages.iter().any(|m| m.contains("Instructions::get()?")));
}

#[test]
fn test_ignores_non_sysvar_accounts() {
    let mut detector = SysvarAccountDetector::default();

    let code_with_non_sysvar = r#"
        use anchor_lang::prelude::*;

        #[derive(Accounts)]
        pub struct MyContext<'info> {
            /// CHECK: Regular AccountInfo, not Sysvar<T>
            pub instructions: AccountInfo<'info>,
            #[account(mut)]
            pub user: Account<'info, User>,
        }
    "#;

    let diagnostics = detector.analyze(code_with_non_sysvar, None);
    assert_eq!(diagnostics.len(), 0); // Should not detect non-Sysvar account types
}

#[test]
fn test_ignores_non_accounts_structs() {
    let mut detector = SysvarAccountDetector::default();

    let code_with_mixed_structs = r#"
        use anchor_lang::prelude::*;

        // This should be ignored (no #[derive(Accounts)])
        pub struct RegularStruct {
            pub clock: Sysvar<'info, Clock>,
        }

        #[derive(Accounts)]
        pub struct ValidAccounts<'info> {
            pub clock: Sysvar<'info, Clock>,
        }

        // This should also be ignored
        #[account]
        pub struct DataStruct {
            pub timestamp: i64,
        }
    "#;

    let diagnostics = detector.analyze(code_with_mixed_structs, None);
    assert_eq!(diagnostics.len(), 1); // Only the Accounts struct should be flagged
}

#[test]
fn test_ignores_regular_accounts() {
    let mut detector = SysvarAccountDetector::default();

    let code_with_regular_accounts = r#"
        use anchor_lang::prelude::*;

        #[derive(Accounts)]
        pub struct MyContext<'info> {
            #[account(mut)]
            pub user: Account<'info, User>,
            pub authority: Signer<'info>,
            pub system_program: Program<'info, System>,
        }

        #[account]
        pub struct User {
            pub balance: u64,
        }
    "#;

    let diagnostics = detector.analyze(code_with_regular_accounts, None);
    assert_eq!(diagnostics.len(), 0); // No sysvars to detect
}

#[test]
fn test_real_world_anchor_pattern() {
    let mut detector = SysvarAccountDetector::default();

    let real_world_code = r#"
        use anchor_lang::prelude::*;

        #[program]
        pub mod my_program {
            use super::*;

            pub fn initialize_with_timestamp(ctx: Context<InitializeWithTimestamp>) -> Result<()> {
                let current_time = ctx.accounts.clock.unix_timestamp;
                ctx.accounts.user_account.created_at = current_time;
                Ok(())
            }
        }

        #[derive(Accounts)]
        pub struct InitializeWithTimestamp<'info> {
            #[account(init, payer = authority, space = 8 + 32)]
            pub user_account: Account<'info, UserAccount>,
            #[account(mut)]
            pub authority: Signer<'info>,
            pub clock: Sysvar<'info, Clock>,
            pub system_program: Program<'info, System>,
        }

        #[account]
        pub struct UserAccount {
            pub authority: Pubkey,
            pub created_at: i64,
        }
    "#;

    let diagnostics = detector.analyze(real_world_code, None);
    assert_eq!(diagnostics.len(), 1);

    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::WARNING));
    assert!(diagnostic.message.contains("Clock::get()?"));
    assert!(diagnostic.message.contains("more efficient"));
}

#[test]
fn test_invalid_syntax_handling() {
    let mut detector = SysvarAccountDetector::default();

    let invalid_code = r#"
        use anchor_lang::prelude::*;

        #[derive(Accounts)]
        pub struct BrokenStruct<'info> {
            pub clock: Sysvar<'info,
        }
    "#;

    // Should handle invalid syntax gracefully
    let diagnostics = detector.analyze(invalid_code, None);
    assert_eq!(diagnostics.len(), 0);
}

#[test]
fn test_detector_state_isolation() {
    let mut detector1 = SysvarAccountDetector::default();
    let mut detector2 = SysvarAccountDetector::default();

    let code = r#"
        use anchor_lang::prelude::*;

        #[derive(Accounts)]
        pub struct TestAccounts<'info> {
            pub clock: Sysvar<'info, Clock>,
        }
    "#;

    let diagnostics1 = detector1.analyze(code, None);
    let diagnostics2 = detector2.analyze(code, None);

    // Each detector instance should produce the same results
    assert_eq!(diagnostics1.len(), diagnostics2.len());
    assert_eq!(diagnostics1.len(), 1);
}
