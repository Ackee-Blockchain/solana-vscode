use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct Counter {
    pub count: u64,
    pub authority: Pubkey,
}

impl Counter {
    pub fn increment_by(&mut self, amount: u64) {
        // This should trigger the lint - addition in a separate module
        self.count = self.count + amount;
    }
}
