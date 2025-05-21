use anchor_lang::prelude::*;

declare_id!("W7GzUqFd8a1b7AEn4jBDjd2T1p6QzyqunxXvcvf82PM");

#[program]
pub mod test_program {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
