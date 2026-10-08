use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface},
};

use super::shared::{close_token_account, harvest_withheld_fees, transfer_tokens};
use crate::{constants::OFFER_SEED, error::ErrorCode, events::OfferClosed, state::Offer};

#[derive(Accounts)]
pub struct RefundOffer<'info> {
    pub associated_token_program: Program<'info, AssociatedToken>,

    pub token_program_a: Interface<'info, TokenInterface>,

    // Used to create accounts
    pub system_program: Program<'info, System>,

    /// The maker, or anyone once the offer has expired (a crank)
    #[account(mut)]
    pub caller: Signer<'info>,

    #[account(mut)]
    pub maker: SystemAccount<'info>,

    // Writable so withheld Token-2022 transfer fees can be harvested to it
    #[account(mut, mint::token_program = token_program_a)]
    pub token_mint_a: Box<InterfaceAccount<'info, Mint>>,

    // The maker may have closed their token A account since making the offer
    #[account(
        init_if_needed,
        payer = caller,
        associated_token::mint = token_mint_a,
        associated_token::authority = maker,
        associated_token::token_program = token_program_a
    )]
    pub maker_token_account_a: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        close = maker,
        has_one = maker,
        has_one = token_mint_a,
        seeds = [OFFER_SEED, maker.key().as_ref(), offer.id.to_le_bytes().as_ref()],
        bump = offer.bump
    )]
    pub offer: Box<Account<'info, Offer>>,

    #[account(
        mut,
        associated_token::mint = token_mint_a,
        associated_token::authority = offer,
        associated_token::token_program = token_program_a,
    )]
    pub vault: Box<InterfaceAccount<'info, TokenAccount>>,
}

// Handle the refund offer instruction by:
// 1. Returning the tokens from the vault to the maker's account
// 2. Closing the vault and returning the rent to the maker
pub fn refund_offer(context: Context<RefundOffer>) -> Result<()> {
    let offer = &context.accounts.offer;
    let now = Clock::get()?.unix_timestamp;
    require!(
        context.accounts.caller.key() == offer.maker || offer.is_expired(now),
        ErrorCode::RefundNotAllowed
    );

    let maker_key = offer.maker;
    let id_bytes = offer.id.to_le_bytes();
    let offer_account_seeds = &[OFFER_SEED, maker_key.as_ref(), &id_bytes[..], &[offer.bump]];
    let signers_seeds = Some(&offer_account_seeds[..]);

    // Return the tokens from the vault to the maker's account
    let refunded = context.accounts.vault.amount;
    if refunded > 0 {
        transfer_tokens(
            &context.accounts.vault,
            &context.accounts.maker_token_account_a,
            &refunded,
            &context.accounts.token_mint_a,
            &context.accounts.offer.to_account_info(),
            &context.accounts.token_program_a,
            signers_seeds,
        )?;
    }

    harvest_withheld_fees(
        &context.accounts.vault,
        &context.accounts.token_mint_a,
        &context.accounts.token_program_a,
    )?;

    // Close the vault and return the rent to the maker
    close_token_account(
        &context.accounts.vault,
        &context.accounts.maker.to_account_info(),
        &context.accounts.offer.to_account_info(),
        &context.accounts.token_program_a,
        signers_seeds,
    )?;

    emit!(OfferClosed {
        offer: context.accounts.offer.key(),
        filled: false,
        token_a_refunded: refunded
    });
    Ok(())
}
