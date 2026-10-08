use super::shared::{close_token_account, harvest_withheld_fees, transfer_tokens};
use crate::{
    constants::OFFER_SEED,
    error::ErrorCode,
    events::{OfferClosed, OfferTaken},
    pricing::token_b_for,
    state::Offer,
};
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface},
};

#[derive(Accounts)]
pub struct TakeOffer<'info> {
    // Used to manage associated token accounts
    // ie where a wallet holds a specific type of token
    pub associated_token_program: Program<'info, AssociatedToken>,

    pub token_program_a: Interface<'info, TokenInterface>,
    pub token_program_b: Interface<'info, TokenInterface>,

    // Used to create accounts
    pub system_program: Program<'info, System>,

    #[account(mut)]
    pub taker: Signer<'info>,

    #[account(mut)]
    pub maker: SystemAccount<'info>,

    // Writable so withheld Token-2022 transfer fees can be harvested to it
    #[account(mut, mint::token_program = token_program_a)]
    pub token_mint_a: Box<InterfaceAccount<'info, Mint>>,

    #[account(mint::token_program = token_program_b)]
    pub token_mint_b: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        init_if_needed,
        payer = taker,
        associated_token::mint = token_mint_a,
        associated_token::authority = taker,
        associated_token::token_program = token_program_a,
    )]
    pub taker_token_account_a: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        associated_token::mint = token_mint_b,
        associated_token::authority = taker,
        associated_token::token_program = token_program_b,
    )]
    pub taker_token_account_b: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = taker,
        associated_token::mint = token_mint_b,
        associated_token::authority = maker,
        associated_token::token_program = token_program_b,
    )]
    pub maker_token_account_b: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        has_one = maker,
        has_one = token_mint_a,
        has_one = token_mint_b,
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

// Handle the take offer instruction by:
// 1. Sending the taker's share of token A from the vault
// 2. Sending the matching amount of token B from the taker to the maker
// 3. Closing the vault and the offer once everything has been sold
pub fn take_offer(context: Context<TakeOffer>, token_a_amount: u64, max_token_b_amount: u64) -> Result<()> {
    let offer = &context.accounts.offer;
    let now = Clock::get()?.unix_timestamp;
    require!(!offer.is_expired(now), ErrorCode::OfferExpired);
    require!(token_a_amount > 0, ErrorCode::InvalidAmount);
    require!(
        token_a_amount <= offer.token_a_remaining,
        ErrorCode::AmountExceedsRemaining
    );
    require!(
        offer.allow_partial_fills || token_a_amount == offer.token_a_remaining,
        ErrorCode::PartialFillNotAllowed
    );

    let token_b_amount = token_b_for(
        token_a_amount,
        offer.token_a_remaining,
        offer.token_a_offered_amount,
        offer.token_b_wanted_amount,
        offer.token_b_received,
    )
    .ok_or(ErrorCode::InvalidAmount)?;
    require!(token_b_amount <= max_token_b_amount, ErrorCode::SlippageExceeded);
    // Checked up front: a failed token CPI aborts the transaction with the
    // token program's error, so it can't be mapped to ours afterwards
    require!(
        context.accounts.taker_token_account_b.amount >= token_b_amount,
        ErrorCode::InsufficientTakerBalance
    );

    let is_final_fill = token_a_amount == offer.token_a_remaining;

    // Since the Offer account owns the Vault, we will say
    // there is one signer (the offer), with the seeds of the specific offer account
    // We can use these signer seeds to withdraw the token from the vault
    let maker_key = offer.maker;
    let id_bytes = offer.id.to_le_bytes();
    let offer_account_seeds = &[OFFER_SEED, maker_key.as_ref(), &id_bytes[..], &[offer.bump]];
    let signers_seeds = Some(&offer_account_seeds[..]);

    // On the final fill also sweep anything sent to the vault directly, so it can be closed
    let token_a_out = if is_final_fill {
        context.accounts.vault.amount
    } else {
        token_a_amount
    };

    // Withdraw the offered tokens from the vault to the taker
    transfer_tokens(
        &context.accounts.vault,
        &context.accounts.taker_token_account_a,
        &token_a_out,
        &context.accounts.token_mint_a,
        &context.accounts.offer.to_account_info(),
        &context.accounts.token_program_a,
        signers_seeds,
    )?;

    // Send the wanted tokens from the taker to the maker
    if token_b_amount > 0 {
        transfer_tokens(
            &context.accounts.taker_token_account_b,
            &context.accounts.maker_token_account_b,
            &token_b_amount,
            &context.accounts.token_mint_b,
            &context.accounts.taker.to_account_info(),
            &context.accounts.token_program_b,
            None,
        )?;
    }

    let offer = &mut context.accounts.offer;
    offer.token_a_remaining -= token_a_amount;
    offer.token_b_received = offer.token_b_received.saturating_add(token_b_amount);

    emit!(OfferTaken {
        offer: offer.key(),
        taker: context.accounts.taker.key(),
        token_a_amount,
        token_b_amount,
        token_a_remaining: offer.token_a_remaining,
    });

    if is_final_fill {
        harvest_withheld_fees(
            &context.accounts.vault,
            &context.accounts.token_mint_a,
            &context.accounts.token_program_a,
        )?;

        // Close the vault and return the rent to the maker, since they made this account in the first place
        close_token_account(
            &context.accounts.vault,
            &context.accounts.maker.to_account_info(),
            &context.accounts.offer.to_account_info(),
            &context.accounts.token_program_a,
            signers_seeds,
        )?;

        emit!(OfferClosed {
            offer: context.accounts.offer.key(),
            filled: true,
            token_a_refunded: 0
        });
        context
            .accounts
            .offer
            .close(context.accounts.maker.to_account_info())?;
    }

    Ok(())
}
