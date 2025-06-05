use language_server::core::detectors::{
    detector::Detector, detector_config::DetectorConfig, unsafe_math::UnsafeMathDetector,
};
use tower_lsp::lsp_types::DiagnosticSeverity;

#[test]
fn test_detector_metadata() {
    let detector = UnsafeMathDetector::new();

    assert_eq!(detector.id(), "UNSAFE_ARITHMETIC");
    assert_eq!(detector.name(), "Unsafe Math Operations");
    assert_eq!(
        detector.description(),
        "Detects unchecked arithmetic operations that could lead to overflow/underflow vulnerabilities"
    );
    assert_eq!(detector.default_severity(), DiagnosticSeverity::ERROR);
}

#[test]
fn test_should_run_with_anchor_and_math() {
    let detector = UnsafeMathDetector::new();

    // Should run on anchor files with math operations
    let anchor_with_math = r#"
        use anchor_lang::prelude::*;

        #[program]
        pub mod my_program {
            use super::*;

            pub fn transfer(ctx: Context<Transfer>, amount: u64) -> Result<()> {
                let balance = ctx.accounts.user.balance + amount;
                Ok(())
            }
        }
    "#;
    assert!(detector.should_run(anchor_with_math));

    // Should run on anchor_spl files with math operations
    let anchor_spl_with_math = r#"
        use anchor_lang::prelude::*;
        use anchor_spl::token::*;

        #[program]
        pub mod token_program {
            use super::*;

            pub fn mint_tokens(ctx: Context<MintTokens>, amount: u64) -> Result<()> {
                let new_supply = ctx.accounts.mint.supply * amount;
                Ok(())
            }
        }
    "#;
    assert!(detector.should_run(anchor_spl_with_math));
}

#[test]
fn test_should_not_run_without_anchor() {
    let detector = UnsafeMathDetector::new();

    // Should not run on files without anchor imports
    let no_anchor = r#"
        fn test() {
            let result = a + b;
        }
    "#;
    assert!(!detector.should_run(no_anchor));
}

#[test]
fn test_should_not_run_without_math() {
    let detector = UnsafeMathDetector::new();

    // Should not run on anchor files without math operations
    let anchor_no_math = r#"
        use anchor_lang::prelude::*;

        #[program]
        pub mod my_program {
            use super::*;

            pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
                ctx.accounts.user.authority = ctx.accounts.authority.key();
                Ok(())
            }
        }
    "#;
    assert!(!detector.should_run(anchor_no_math));
}

#[test]
fn test_detects_addition_in_instruction() {
    let mut detector = UnsafeMathDetector::new();

    let code_with_addition = r#"
        use anchor_lang::prelude::*;

        #[program]
        pub mod vault_program {
            use super::*;

            pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
                ctx.accounts.vault.balance = ctx.accounts.vault.balance + amount;
                Ok(())
            }
        }

        #[derive(Accounts)]
        pub struct Deposit<'info> {
            #[account(mut)]
            pub vault: Account<'info, Vault>,
            pub user: Signer<'info>,
        }

        #[account]
        pub struct Vault {
            pub balance: u64,
            pub authority: Pubkey,
        }
    "#;

    let diagnostics = detector.analyze(code_with_addition);
    assert_eq!(diagnostics.len(), 1);

    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::ERROR));
    assert!(
        diagnostic
            .message
            .contains("Unchecked arithmetic operation detected")
    );
    assert!(diagnostic.message.contains("checked_add()"));
}

#[test]
fn test_detects_multiple_arithmetic_operations() {
    let mut detector = UnsafeMathDetector::new();

    let code_with_multiple_operations = r#"
        use anchor_lang::prelude::*;

        #[program]
        pub mod defi_program {
            use super::*;

            pub fn calculate_rewards(ctx: Context<CalculateRewards>, stake_amount: u64, duration: u64) -> Result<()> {
                let base_reward = stake_amount + ctx.accounts.pool.base_rate;
                let time_bonus = duration + ctx.accounts.pool.time_multiplier;
                let total_reward = base_reward + time_bonus;

                ctx.accounts.user.rewards = total_reward;
                Ok(())
            }
        }

        #[derive(Accounts)]
        pub struct CalculateRewards<'info> {
            #[account(mut)]
            pub user: Account<'info, User>,
            pub pool: Account<'info, Pool>,
        }

        #[account]
        pub struct User {
            pub rewards: u64,
            pub stake_amount: u64,
        }

        #[account]
        pub struct Pool {
            pub base_rate: u64,
            pub time_multiplier: u64,
        }
    "#;

    let diagnostics = detector.analyze(code_with_multiple_operations);
    assert_eq!(diagnostics.len(), 3);

    for diagnostic in &diagnostics {
        assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::ERROR));
        assert!(
            diagnostic
                .message
                .contains("Unchecked arithmetic operation detected")
        );
    }
}

#[test]
fn test_nested_addition_in_anchor_context() {
    let mut detector = UnsafeMathDetector::new();

    let code_with_nested_additions = r#"
        use anchor_lang::prelude::*;

        #[program]
        pub mod complex_math {
            use super::*;

            pub fn complex_calculation(ctx: Context<ComplexCalc>, a: u64, b: u64, c: u64, d: u64) -> Result<()> {
                let result = (a + b) + (c + d);
                ctx.accounts.storage.value = result;
                Ok(())
            }
        }

        #[derive(Accounts)]
        pub struct ComplexCalc<'info> {
            #[account(mut)]
            pub storage: Account<'info, Storage>,
        }

        #[account]
        pub struct Storage {
            pub value: u64,
        }
    "#;

    let diagnostics = detector.analyze(code_with_nested_additions);
    // Should detect 3 additions: a+b, c+d, and (a+b)+(c+d)
    assert_eq!(diagnostics.len(), 3);
}

#[test]
fn test_no_detection_without_addition() {
    let mut detector = UnsafeMathDetector::new();

    let code_without_addition = r#"
        use anchor_lang::prelude::*;

        #[program]
        pub mod safe_program {
            use super::*;

            pub fn safe_operation(ctx: Context<SafeOp>) -> Result<()> {
                ctx.accounts.user.last_update = Clock::get()?.unix_timestamp;
                ctx.accounts.user.status = UserStatus::Active;
                Ok(())
            }
        }

        #[derive(Accounts)]
        pub struct SafeOp<'info> {
            #[account(mut)]
            pub user: Account<'info, User>,
        }

        #[account]
        pub struct User {
            pub last_update: i64,
            pub status: UserStatus,
        }

        #[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
        pub enum UserStatus {
            Active,
            Inactive,
        }
    "#;

    let diagnostics = detector.analyze(code_without_addition);
    assert_eq!(diagnostics.len(), 0);
}

#[test]
fn test_invalid_syntax_handling() {
    let mut detector = UnsafeMathDetector::new();

    let invalid_code = r#"
        use anchor_lang::prelude::*;

        #[program]
        pub mod broken_program {
            use super::*;

            pub fn broken_function(ctx: Context<BrokenCtx> {
                let result = a + ;
            }
        }
    "#;

    // Should handle invalid syntax gracefully and return empty diagnostics
    let diagnostics = detector.analyze(invalid_code);
    assert_eq!(diagnostics.len(), 0);
}

#[test]
fn test_addition_in_different_anchor_contexts() {
    let mut detector = UnsafeMathDetector::new();

    let code_with_various_contexts = r#"
        use anchor_lang::prelude::*;

        #[program]
        pub mod multi_context_program {
            use super::*;

            pub fn instruction_handler(ctx: Context<MyContext>, amount: u64) -> Result<()> {
                ctx.accounts.account.balance = ctx.accounts.account.balance + amount;
                Ok(())
            }
        }

        impl<'info> MyContext<'info> {
            fn helper_method(&self, value: u64) -> u64 {
                let result = self.account.balance + value;
                result
            }
        }

        #[derive(Accounts)]
        pub struct MyContext<'info> {
            #[account(mut)]
            pub account: Account<'info, MyAccount>,
        }

        #[account]
        pub struct MyAccount {
            pub balance: u64,
        }

        mod utils {
            pub fn calculate_fee(base: u64, rate: u64) -> u64 {
                base + rate
            }
        }
    "#;

    let diagnostics = detector.analyze(code_with_various_contexts);
    assert_eq!(diagnostics.len(), 3);
}

#[test]
fn test_token_transfer_with_unsafe_math() {
    let mut detector = UnsafeMathDetector::new();

    let token_program_code = r#"
        use anchor_lang::prelude::*;
        use anchor_spl::token::{self, Token, TokenAccount, Transfer};

        #[program]
        pub mod token_vault {
            use super::*;

            pub fn deposit_tokens(ctx: Context<DepositTokens>, amount: u64) -> Result<()> {
                // Unsafe: could overflow
                ctx.accounts.vault.total_deposits = ctx.accounts.vault.total_deposits + amount;

                let cpi_accounts = Transfer {
                    from: ctx.accounts.user_token_account.to_account_info(),
                    to: ctx.accounts.vault_token_account.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                };

                let cpi_program = ctx.accounts.token_program.to_account_info();
                let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);

                token::transfer(cpi_ctx, amount)?;
                Ok(())
            }
        }

        #[derive(Accounts)]
        pub struct DepositTokens<'info> {
            #[account(mut)]
            pub vault: Account<'info, Vault>,
            #[account(mut)]
            pub vault_token_account: Account<'info, TokenAccount>,
            #[account(mut)]
            pub user_token_account: Account<'info, TokenAccount>,
            pub user: Signer<'info>,
            pub token_program: Program<'info, Token>,
        }

        #[account]
        pub struct Vault {
            pub total_deposits: u64,
            pub authority: Pubkey,
        }
    "#;

    let diagnostics = detector.analyze(token_program_code);
    // Should detect the unsafe addition in total_deposits calculation
    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn test_detector_state_isolation() {
    let mut detector1 = UnsafeMathDetector::new();
    let mut detector2 = UnsafeMathDetector::new();

    let code = r#"
        use anchor_lang::prelude::*;

        #[program]
        pub mod test_program {
            use super::*;

            pub fn test_instruction(ctx: Context<TestCtx>, amount: u64) -> Result<()> {
                let result = ctx.accounts.account.value + amount;
                Ok(())
            }
        }

        #[derive(Accounts)]
        pub struct TestCtx<'info> {
            #[account(mut)]
            pub account: Account<'info, TestAccount>,
        }

        #[account]
        pub struct TestAccount {
            pub value: u64,
        }
    "#;

    let diagnostics1 = detector1.analyze(code);
    let diagnostics2 = detector2.analyze(code);

    // Each detector instance should produce the same results
    assert_eq!(diagnostics1.len(), diagnostics2.len());
    assert_eq!(diagnostics1.len(), 1);
}

#[test]
fn test_custom_patterns_basic() {
    let config =
        DetectorConfig::with_patterns(vec!["todo!()".to_string(), "unimplemented!()".to_string()]);
    let mut detector = UnsafeMathDetector::with_config(config);

    let code_with_custom_patterns = r#"
        use anchor_lang::prelude::*;

        #[program]
        pub mod test_program {
            use super::*;

            pub fn incomplete_function(ctx: Context<Test>) -> Result<()> {
                todo!(); // This should be detected
                Ok(())
            }

            pub fn another_function(ctx: Context<Test>) -> Result<()> {
                unimplemented!(); // This should also be detected
                Ok(())
            }
        }
    "#;

    let diagnostics = detector.analyze(code_with_custom_patterns);

    // Should detect 2 custom patterns
    let custom_diagnostics: Vec<_> = diagnostics
        .iter()
        .filter(|d| {
            if let Some(code) = &d.code {
                match code {
                    tower_lsp::lsp_types::NumberOrString::String(s) => s.contains("CUSTOM"),
                    _ => false,
                }
            } else {
                false
            }
        })
        .collect();

    assert_eq!(custom_diagnostics.len(), 2);

    // Check that custom pattern diagnostics have correct format
    for diagnostic in custom_diagnostics {
        assert!(diagnostic.message.contains("Custom pattern"));
        assert!(if let Some(code) = &diagnostic.code {
            match code {
                tower_lsp::lsp_types::NumberOrString::String(s) => {
                    s.contains("UNSAFE_ARITHMETIC_CUSTOM")
                }
                _ => false,
            }
        } else {
            false
        });
    }
}

#[test]
fn test_custom_patterns_with_severity_override() {
    let config = DetectorConfig {
        enabled: true,
        severity_override: Some(DiagnosticSeverity::INFORMATION),
        custom_patterns: vec!["println!".to_string()],
    };
    let mut detector = UnsafeMathDetector::with_config(config);

    let code_with_debug_prints = r#"
        use anchor_lang::prelude::*;

        #[program]
        pub mod debug_program {
            use super::*;

            pub fn debug_function(ctx: Context<Test>) -> Result<()> {
                println!("Debug message"); // Should be detected with INFO severity
                let result = 1 + 2; // Should be detected with INFO severity (overridden)
                Ok(())
            }
        }
    "#;

    let diagnostics = detector.analyze(code_with_debug_prints);

    // Should have at least 2 diagnostics (1 custom pattern + 1 arithmetic)
    assert!(diagnostics.len() >= 2);

    // All diagnostics should have INFO severity due to override
    for diagnostic in &diagnostics {
        assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::INFORMATION));
    }
}

#[test]
fn test_custom_patterns_should_run_logic() {
    let config = DetectorConfig::with_patterns(vec!["custom_pattern".to_string()]);
    let detector = UnsafeMathDetector::with_config(config);

    // Should run if custom patterns are configured, even without math operations
    let anchor_no_math = r#"
        use anchor_lang::prelude::*;

        #[program]
        pub mod simple_program {
            use super::*;

            pub fn simple_function(ctx: Context<Test>) -> Result<()> {
                Ok(())
            }
        }
    "#;

    assert!(detector.should_run(anchor_no_math));

    // Should not run without anchor imports, even with custom patterns
    let no_anchor = r#"
        fn test() {
            custom_pattern();
        }
    "#;

    assert!(!detector.should_run(no_anchor));
}

#[test]
fn test_custom_patterns_empty_list() {
    let config = DetectorConfig::with_patterns(vec![]);
    let mut detector = UnsafeMathDetector::with_config(config);

    let code = r#"
        use anchor_lang::prelude::*;

        #[program]
        pub mod test_program {
            use super::*;

            pub fn test_function(ctx: Context<Test>) -> Result<()> {
                todo!(); // Should not be detected (no custom patterns)
                let result = 1 + 2; // Should be detected (default behavior)
                Ok(())
            }
        }
    "#;

    let diagnostics = detector.analyze(code);

    // Should only detect the arithmetic operation, not the todo!()
    assert_eq!(diagnostics.len(), 1);
    assert!(if let Some(code) = &diagnostics[0].code {
        match code {
            tower_lsp::lsp_types::NumberOrString::String(s) => !s.contains("CUSTOM"),
            _ => true,
        }
    } else {
        true
    });
}

#[test]
fn test_custom_patterns_solana_specific() {
    let config = DetectorConfig::with_patterns(vec![
        "invoke_unchecked".to_string(),
        "try_from_unchecked".to_string(),
    ]);
    let mut detector = UnsafeMathDetector::with_config(config);

    let code_with_solana_patterns = r#"use anchor_lang::prelude::*;
        use solana_program::program::invoke_unchecked;

        #[program]
        pub mod risky_program {
            use super::*;

            pub fn risky_invoke(ctx: Context<Test>) -> Result<()> {
                invoke_unchecked(&instruction, &accounts)?; // Should be detected

                let account = AccountLoader::try_from_unchecked(&account_info)?; // Should be detected

                Ok(())
            }
        }
    "#;

    let diagnostics = detector.analyze(code_with_solana_patterns);

    // Should detect only the patterns in actual code, not in imports
    let custom_diagnostics: Vec<_> = diagnostics
        .iter()
        .filter(|d| {
            if let Some(code) = &d.code {
                match code {
                    tower_lsp::lsp_types::NumberOrString::String(s) => s.contains("CUSTOM"),
                    _ => false,
                }
            } else {
                false
            }
        })
        .collect();

    assert_eq!(custom_diagnostics.len(), 2); // invoke_unchecked in code + try_from_unchecked in code
}

#[test]
fn test_custom_patterns_position_accuracy() {
    let config = DetectorConfig::with_patterns(vec!["PATTERN".to_string()]);
    let mut detector = UnsafeMathDetector::with_config(config);

    let code = r#"use anchor_lang::prelude::*;

        #[program]
        pub mod test {
            use super::*;

            pub fn test() {
                PATTERN; // Line 8, should be detected here
            }
        }
}"#;

    let diagnostics = detector.analyze(code);

    let custom_diagnostic = diagnostics
        .iter()
        .find(|d| {
            if let Some(code) = &d.code {
                match code {
                    tower_lsp::lsp_types::NumberOrString::String(s) => s.contains("CUSTOM"),
                    _ => false,
                }
            } else {
                false
            }
        })
        .expect("Should find custom pattern diagnostic");

    // Should be on line 8 (0-indexed = 7)
    assert_eq!(custom_diagnostic.range.start.line, 7);

    // Should span the length of "PATTERN"
    let pattern_length = "PATTERN".len() as u32;
    assert_eq!(
        custom_diagnostic.range.end.character - custom_diagnostic.range.start.character,
        pattern_length
    );
}

#[test]
fn test_custom_patterns_with_default_detection() {
    let config = DetectorConfig::with_patterns(vec!["debug!".to_string()]);
    let mut detector = UnsafeMathDetector::with_config(config);

    let code = r#"
        use anchor_lang::prelude::*;

        #[program]
        pub mod mixed_program {
            use super::*;

            pub fn mixed_function(ctx: Context<Test>) -> Result<()> {
                debug!("Starting calculation"); // Custom pattern
                let result = 1 + 2; // Default detection
                debug!("Calculation complete"); // Custom pattern
                Ok(())
            }
        }
    "#;

    let diagnostics = detector.analyze(code);

    // Should have 3 diagnostics total: 2 custom + 1 arithmetic
    assert_eq!(diagnostics.len(), 3);

    let custom_count = diagnostics
        .iter()
        .filter(|d| {
            if let Some(code) = &d.code {
                match code {
                    tower_lsp::lsp_types::NumberOrString::String(s) => s.contains("CUSTOM"),
                    _ => false,
                }
            } else {
                false
            }
        })
        .count();

    let arithmetic_count = diagnostics
        .iter()
        .filter(|d| {
            if let Some(code) = &d.code {
                match code {
                    tower_lsp::lsp_types::NumberOrString::String(s) => s == "UNSAFE_ARITHMETIC",
                    _ => false,
                }
            } else {
                false
            }
        })
        .count();

    assert_eq!(custom_count, 2);
    assert_eq!(arithmetic_count, 1);
}

#[test]
fn test_custom_patterns_ignore_imports() {
    let config = DetectorConfig::with_patterns(vec!["test_pattern".to_string()]);
    let mut detector = UnsafeMathDetector::with_config(config);

    let code_with_pattern_in_import = r#"
        use anchor_lang::prelude::*;
        use some_crate::test_pattern; // Should be ignored

        #[program]
        pub mod test_program {
            use super::*;

            pub fn test_function(ctx: Context<Test>) -> Result<()> {
                test_pattern(); // Should be detected
                Ok(())
            }
        }
    "#;

    let diagnostics = detector.analyze(code_with_pattern_in_import);

    // Should detect only the pattern in actual code, not in import
    let custom_diagnostics: Vec<_> = diagnostics
        .iter()
        .filter(|d| {
            if let Some(code) = &d.code {
                match code {
                    tower_lsp::lsp_types::NumberOrString::String(s) => s.contains("CUSTOM"),
                    _ => false,
                }
            } else {
                false
            }
        })
        .collect();

    assert_eq!(custom_diagnostics.len(), 1); // Only the one in actual code
}

#[test]
fn test_custom_patterns_ignore_comments() {
    let config = DetectorConfig::with_patterns(vec!["test_pattern".to_string()]);
    let mut detector = UnsafeMathDetector::with_config(config);

    let code_with_pattern_in_comments = r#"
        use anchor_lang::prelude::*;

        #[program]
        pub mod test_program {
            use super::*;

            pub fn test_function(ctx: Context<Test>) -> Result<()> {
                // This test_pattern should be ignored
                /* This test_pattern should also be ignored */
                /*
                 * Multi-line comment with test_pattern
                 * should be ignored too
                 */
                test_pattern(); // Should be detected
                Ok(())
            }
        }
    "#;

    let diagnostics = detector.analyze(code_with_pattern_in_comments);

    // Should detect only the pattern in actual code, not in comments
    let custom_diagnostics: Vec<_> = diagnostics
        .iter()
        .filter(|d| {
            if let Some(code) = &d.code {
                match code {
                    tower_lsp::lsp_types::NumberOrString::String(s) => s.contains("CUSTOM"),
                    _ => false,
                }
            } else {
                false
            }
        })
        .collect();

    assert_eq!(custom_diagnostics.len(), 1); // Only the one in actual code
}
