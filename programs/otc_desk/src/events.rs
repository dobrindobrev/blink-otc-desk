use anchor_lang::prelude::*;

#[event]
pub struct OfferMade {
    pub offer: Pubkey,
    pub maker: Pubkey,
    pub token_mint_a: Pubkey,
    pub token_mint_b: Pubkey,
    pub token_a_offered_amount: u64,
    pub token_b_wanted_amount: u64,
    pub allow_partial_fills: bool,
    pub expires_at: i64,
}

#[event]
pub struct OfferTaken {
    pub offer: Pubkey,
    pub taker: Pubkey,
    pub token_a_amount: u64,
    pub token_b_amount: u64,
    pub token_a_remaining: u64,
}

#[event]
pub struct OfferClosed {
    pub offer: Pubkey,
    /// true when every token was sold, false when refunded
    pub filled: bool,
    pub token_a_refunded: u64,
}
