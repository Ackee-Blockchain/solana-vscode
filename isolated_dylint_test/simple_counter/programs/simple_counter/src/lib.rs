use anchor_lang::prelude::*;

declare_id!("2TsTuhv4tT5XznfX4N9ti7D56g1JQwqjBePtbzZixrJH");

pub mod state;
pub mod utils;

use state::*;
use utils::*;

#[program]
pub mod simple_counter {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let counter = &mut ctx.accounts.counter;
        counter.count = 0;
        counter.authority = ctx.accounts.authority.key();
        msg!("Counter initialized!");
        Ok(())
    }

    pub fn increment(ctx: Context<Increment>) -> Result<()> {
        let counter = &mut ctx.accounts.counter;

        // This should trigger the lint - addition operation
        counter.count = counter.count + 1;

        msg!("Counter incremented to: {}", counter.count);
        Ok(())
    }

    pub fn add_value(ctx: Context<Increment>, value: u64) -> Result<()> {
        let counter = &mut ctx.accounts.counter;

        // This should also trigger the lint - addition with custom value
        let new_count = counter.count + value;
        counter.count = new_count;

        // Call helper function with addition
        let result = calculate_double(value);
        msg!(
            "Added {} to counter. New value: {}. Double: {}",
            value,
            counter.count,
            result
        );

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + Counter::INIT_SPACE
    )]
    pub counter: Account<'info, Counter>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Increment<'info> {
    #[account(mut)]
    pub counter: Account<'info, Counter>,
    pub authority: Signer<'info>,
}
