use language_server::core::detectors::{detector::Detector, unsafe_math::UnsafeMathDetector};
use tower_lsp::lsp_types::DiagnosticSeverity;

#[test]
fn test_detector_metadata() {
    let detector = UnsafeMathDetector::new();

    assert_eq!(detector.id(), "UNSAFE_ARITHMETIC");
    assert_eq!(detector.name(), "Unsafe Math Operations");
    assert_eq!(detector.description(), "Detects unchecked arithmetic operations that could lead to overflow/underflow vulnerabilities");
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
    assert!(diagnostic.message.contains("Unchecked arithmetic operation detected"));
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
        assert!(diagnostic.message.contains("Unchecked arithmetic operation detected"));
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
