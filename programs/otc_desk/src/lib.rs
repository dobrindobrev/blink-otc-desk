//! OTC Desk — peer-to-peer token swaps with partial fills and expiry.
//!
//! A maker locks token A in a vault owned by an offer PDA and names a price
//! in token B. Takers fill all or (if allowed) part of the offer at that
//! price. Unfilled tokens go back to the maker on cancel, or to anyone's
//! crank once the offer has expired.

use anchor_lang::prelude::*;

pub mod constants;
pub mod error;
pub mod events;
pub mod handlers;
pub mod pricing;
pub mod state;

use handlers::*;

declare_id!("6se56Bge1Gg9YD9yK7EUe8r2wmU9FJTWKkhseSKiMbQy");

#[program]
pub mod otc_desk {
    use super::*;

    /// Lock `token_a_offered_amount` of token A and ask for `token_b_wanted_amount`
    /// of token B in return. `expires_at` is a unix timestamp, or 0 for never.
    pub fn make_offer(
        context: Context<MakeOffer>,
        id: u64,
        token_a_offered_amount: u64,
        token_b_wanted_amount: u64,
        allow_partial_fills: bool,
        expires_at: i64,
    ) -> Result<()> {
        handlers::make_offer::make_offer(
            context,
            id,
            token_a_offered_amount,
            token_b_wanted_amount,
            allow_partial_fills,
            expires_at,
        )
    }

    /// Buy `token_a_amount` of token A at the offer's price, paying at most
    /// `max_token_b_amount` of token B.
    pub fn take_offer(
        context: Context<TakeOffer>,
        token_a_amount: u64,
        max_token_b_amount: u64,
    ) -> Result<()> {
        handlers::take_offer::take_offer(context, token_a_amount, max_token_b_amount)
    }

    /// Return the unfilled tokens to the maker and close the offer. The maker
    /// can call this at any time; anyone can call it once the offer has expired.
    pub fn refund_offer(context: Context<RefundOffer>) -> Result<()> {
        handlers::refund_offer::refund_offer(context)
    }
}
