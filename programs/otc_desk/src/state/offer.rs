use anchor_lang::prelude::*;

use crate::constants::NO_EXPIRY;

/// An offer to sell token A for token B, at PDA `["offer", maker, id]`.
#[account]
#[derive(InitSpace)]
pub struct Offer {
    /// Identifier of the offer, unique per maker
    pub id: u64,
    /// Who made the offer
    pub maker: Pubkey,
    /// The token mint of the token being offered
    pub token_mint_a: Pubkey,
    /// The token mint of the token wanted
    pub token_mint_b: Pubkey,
    /// Token A that actually reached the vault (after any Token-2022 transfer fee)
    pub token_a_offered_amount: u64,
    /// Total token B wanted for all of `token_a_offered_amount`
    pub token_b_wanted_amount: u64,
    /// Token A still available to takers
    pub token_a_remaining: u64,
    /// Token B paid to the maker so far
    pub token_b_received: u64,
    pub allow_partial_fills: bool,
    /// Unix timestamp after which the offer can't be taken, or 0 for never
    pub expires_at: i64,
    /// Used to calculate the address for this account, we save it as a performance optimization
    pub bump: u8,
}

impl Offer {
    pub fn is_expired(&self, now: i64) -> bool {
        self.expires_at != NO_EXPIRY && now >= self.expires_at
    }
}
