use language_server::core::detectors::{
    detector::Detector, manual_lamports_zeroing::ManualLamportsZeroingDetector,
};
use tower_lsp::lsp_types::DiagnosticSeverity;

#[test]
fn test_detector_metadata() {
    let detector = ManualLamportsZeroingDetector::default();

    assert_eq!(detector.id(), "MANUAL_LAMPORTS_ZEROING");
    assert_eq!(detector.name(), "Manual Lamports Zeroing");
    assert_eq!(
        detector.description(),
        "Detects manual lamports zeroing which can lead to incomplete account closure and potential security vulnerabilities"
    );
    assert_eq!(detector.default_severity(), DiagnosticSeverity::ERROR);
}

#[test]
fn test_detects_direct_lamports_assignment() {
    let mut detector = ManualLamportsZeroingDetector::default();

    let code_with_direct_assignment = r#"
        use anchor_lang::prelude::*;

        #[program]
        pub mod vulnerable_program {
            use super::*;

            pub fn close_account(ctx: Context<CloseAccount>) -> Result<()> {
                ctx.accounts.target_account.lamports = 0;
                Ok(())
            }
        }

        #[derive(Accounts)]
        pub struct CloseAccount<'info> {
            #[account(mut)]
            pub target_account: AccountInfo<'info>,
            pub authority: Signer<'info>,
        }
    "#;

    let diagnostics = detector.analyze(code_with_direct_assignment, None);
    assert_eq!(diagnostics.len(), 1);

    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::ERROR));
    assert!(
        diagnostic
            .message
            .contains("Manual lamports zeroing detected")
    );
    assert!(diagnostic.message.contains("close"));
}

#[test]
fn test_detects_set_lamports_method() {
    let mut detector = ManualLamportsZeroingDetector::default();

    let code_with_set_lamports = r#"
        use solana_program::prelude::*;

        pub fn process_instruction(
            program_id: &Pubkey,
            accounts: &[AccountInfo],
            instruction_data: &[u8],
        ) -> ProgramResult {
            let account = &accounts[0];
            account.set_lamports(0);
            Ok(())
        }
    "#;

    let diagnostics = detector.analyze(code_with_set_lamports, None);
    assert_eq!(diagnostics.len(), 1);

    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::ERROR));
    assert!(
        diagnostic
            .message
            .contains("Manual lamports zeroing detected")
    );
}

#[test]
fn test_no_detection_for_safe_patterns() {
    let mut detector = ManualLamportsZeroingDetector::default();

    let safe_code = r#"
        use anchor_lang::prelude::*;

        #[program]
        pub mod safe_program {
            use super::*;

            pub fn close_account(ctx: Context<CloseAccount>) -> Result<()> {
                // Safe: using proper close mechanism
                ctx.accounts.target_account.close(ctx.accounts.destination.to_account_info())?;
                Ok(())
            }

            pub fn transfer_lamports(ctx: Context<TransferLamports>, amount: u64) -> Result<()> {
                // Safe: transferring to another account
                **ctx.accounts.from.lamports.borrow_mut() -= amount;
                **ctx.accounts.to.lamports.borrow_mut() += amount;
                Ok(())
            }
        }
    "#;

    let diagnostics = detector.analyze(safe_code, None);
    assert_eq!(diagnostics.len(), 0);
}

#[test]
fn test_detects_multiple_violations() {
    let mut detector = ManualLamportsZeroingDetector::default();

    let code_with_multiple_violations = r#"
        use anchor_lang::prelude::*;

        #[program]
        pub mod vulnerable_program {
            use super::*;

            pub fn bad_close1(ctx: Context<CloseAccount>) -> Result<()> {
                ctx.accounts.account1.lamports = 0;
                Ok(())
            }

            pub fn bad_close2(ctx: Context<CloseAccount>) -> Result<()> {
                ctx.accounts.account2.set_lamports(0);
                Ok(())
            }

            pub fn bad_close3(ctx: Context<CloseAccount>) -> Result<()> {
                let account = &ctx.accounts.account3;
                account.lamports = 0;
                Ok(())
            }
        }
    "#;

    let diagnostics = detector.analyze(code_with_multiple_violations, None);
    assert_eq!(diagnostics.len(), 3);

    for diagnostic in &diagnostics {
        assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::ERROR));
        assert!(
            diagnostic
                .message
                .contains("Manual lamports zeroing detected")
        );
    }
}

#[test]
fn test_ignores_non_zero_assignments() {
    let mut detector = ManualLamportsZeroingDetector::default();

    let code_with_non_zero = r#"
        use anchor_lang::prelude::*;

        pub fn process(ctx: Context<ProcessContext>) -> Result<()> {
            // These should not be flagged
            ctx.accounts.account.lamports = 1000;
            ctx.accounts.account.set_lamports(500);

            let zero = 0;
            let other_field = 0; // Not lamports

            Ok(())
        }
    "#;

    let diagnostics = detector.analyze(code_with_non_zero, None);
    assert_eq!(diagnostics.len(), 0);
}

#[test]
fn test_complex_lamports_patterns() {
    let mut detector = ManualLamportsZeroingDetector::default();

    let complex_code = r#"
        use anchor_lang::prelude::*;
        use anchor_spl::token::{Token, TokenAccount};

        #[program]
        pub mod token_program {
            use super::*;

            pub fn vulnerable_close(ctx: Context<VulnerableClose>) -> Result<()> {
                // Vulnerable: manual zeroing
                ctx.accounts.token_account.to_account_info().lamports = 0;
                Ok(())
            }

            pub fn safe_close(ctx: Context<SafeClose>) -> Result<()> {
                Ok(())
            }

            pub fn another_vulnerable(ctx: Context<AnotherVulnerable>) -> Result<()> {
                let account_info = ctx.accounts.account.to_account_info();
                // Vulnerable: setting to zero
                account_info.set_lamports(0);
                Ok(())
            }
        }

        #[derive(Accounts)]
        pub struct VulnerableClose<'info> {
            #[account(mut, close = destination)]
            pub token_account: Account<'info, TokenAccount>,
            #[account(mut)]
            pub destination: AccountInfo<'info>,
            pub authority: Signer<'info>,
            pub token_program: Program<'info, Token>,
        }

        #[derive(Accounts)]
        pub struct SafeClose<'info> {
            #[account(mut, close = destination)]
            pub token_account: Account<'info, TokenAccount>,
            #[account(mut)]
            pub destination: AccountInfo<'info>,
            pub authority: Signer<'info>,
        }

        #[derive(Accounts)]
        pub struct AnotherVulnerable<'info> {
            #[account(mut)]
            pub account: AccountInfo<'info>,
            pub authority: Signer<'info>,
        }
    "#;

    let diagnostics = detector.analyze(complex_code, None);
    assert_eq!(diagnostics.len(), 2); // Should detect 2 vulnerable patterns
}

#[test]
fn test_invalid_syntax_handling() {
    let mut detector = ManualLamportsZeroingDetector::default();

    let invalid_code = r#"
        use anchor_lang::prelude::*;

        pub fn broken_function() {
            account.lamports = ;
            account.set_lamports(
        }
    "#;

    // Should handle invalid syntax gracefully
    let diagnostics = detector.analyze(invalid_code, None);
    assert_eq!(diagnostics.len(), 0);
}

#[test]
fn test_detector_state_isolation() {
    let mut detector1 = ManualLamportsZeroingDetector::default();
    let mut detector2 = ManualLamportsZeroingDetector::default();

    let code = r#"
        use anchor_lang::prelude::*;

        pub fn test_function(account: &AccountInfo) {
            account.lamports = 0;
        }
    "#;

    let diagnostics1 = detector1.analyze(code, None);
    let diagnostics2 = detector2.analyze(code, None);

    // Each detector instance should produce the same results
    assert_eq!(diagnostics1.len(), diagnostics2.len());
    assert_eq!(diagnostics1.len(), 1);
}

#[test]
fn test_real_world_vulnerability_patterns() {
    let mut detector = ManualLamportsZeroingDetector::default();

    let real_world_vulnerable = r#"
        use anchor_lang::prelude::*;

        #[program]
        pub mod vault_program {
            use super::*;

            // Common vulnerability: manual account closure
            pub fn close_vault(ctx: Context<CloseVault>) -> Result<()> {
                let vault = &ctx.accounts.vault;
                let destination = &ctx.accounts.destination;

                // Transfer remaining balance
                let balance = vault.to_account_info().lamports();
                **destination.lamports.borrow_mut() += balance;

                // VULNERABLE: Manual zeroing instead of proper close
                vault.to_account_info().lamports = 0;

                Ok(())
            }

            // Another common pattern
            pub fn emergency_drain(ctx: Context<EmergencyDrain>) -> Result<()> {
                let account = &ctx.accounts.emergency_account;

                // VULNERABLE: Direct zeroing
                account.set_lamports(0);

                Ok(())
            }
        }

        #[derive(Accounts)]
        pub struct CloseVault<'info> {
            #[account(mut)]
            pub vault: Account<'info, Vault>,
            #[account(mut)]
            pub destination: AccountInfo<'info>,
            pub authority: Signer<'info>,
        }

        #[derive(Accounts)]
        pub struct EmergencyDrain<'info> {
            #[account(mut)]
            pub emergency_account: AccountInfo<'info>,
            pub admin: Signer<'info>,
        }

        #[account]
        pub struct Vault {
            pub authority: Pubkey,
            pub balance: u64,
        }
    "#;

    let diagnostics = detector.analyze(real_world_vulnerable, None);
    assert_eq!(diagnostics.len(), 2); // Should detect both vulnerable patterns

    for diagnostic in &diagnostics {
        assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::ERROR));
        assert!(
            diagnostic
                .message
                .contains("Manual lamports zeroing detected")
        );
    }
}

#[test]
fn test_detects_borrow_mut_chain_zero() {
    let mut detector = ManualLamportsZeroingDetector::default();

    let code = r#"
        use anchor_lang::prelude::*;

        pub fn bad(ctx: Context<'_>) -> Result<()> {
            // **acct.lamports.borrow_mut() = 0;
            **ctx.accounts.victim.lamports.borrow_mut() = 0;
            Ok(())
        }

        #[derive(Accounts)]
        pub struct _Ctx<'info> {
            #[account(mut)]
            pub victim: AccountInfo<'info>,
        }
    "#;

    let diagnostics = detector.analyze(code, None);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Manual lamports zeroing detected"));
}

#[test]
fn test_detects_try_borrow_mut_lamports_zero() {
    let mut detector = ManualLamportsZeroingDetector::default();

    let code = r#"
        use anchor_lang::prelude::*;

        pub fn bad(ctx: Context<'_>) -> Result<()> {
            // **ctx.accounts.victim.try_borrow_mut_lamports()? = 0;
            **ctx.accounts.victim.try_borrow_mut_lamports()? = 0;
            Ok(())
        }

        #[derive(Accounts)]
        pub struct _Ctx<'info> {
            #[account(mut)]
            pub victim: AccountInfo<'info>,
        }
    "#;

    let diagnostics = detector.analyze(code, None);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Manual lamports zeroing detected"));
}

#[test]
fn test_set_lamports_non_zero_is_ok() {
    let mut d = ManualLamportsZeroingDetector::default();
    let src = r#"
        use solana_program::prelude::*;
        pub fn f(accounts: &[AccountInfo]) -> ProgramResult {
            accounts[0].set_lamports(1);
            Ok(())
        }
    "#;
    assert!(d.analyze(src, None).is_empty());
}

#[test]
fn test_to_account_info_chain_zero() {
    let mut d = ManualLamportsZeroingDetector::default();
    let src = r#"
        use anchor_lang::prelude::*;
        pub fn f(ctx: Context<'_>) -> Result<()> {
            **ctx.accounts.victim.to_account_info().lamports.borrow_mut() = 0;
            Ok(())
        }
        #[derive(Accounts)]
        pub struct _C<'info> { #[account(mut)] pub victim: AccountInfo<'info> }
    "#;
    let diags = d.analyze(src, None);
    assert_eq!(diags.len(), 1);
}