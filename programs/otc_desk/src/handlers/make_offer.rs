use super::shared::transfer_tokens;
use crate::{
    constants::{NO_EXPIRY, OFFER_SEED},
    error::ErrorCode,
    events::OfferMade,
    state::Offer,
};
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface},
};

// See https://www.anchor-lang.com/docs/account-constraints#instruction-attribute
#[derive(Accounts)]
#[instruction(id: u64)]
pub struct MakeOffer<'info> {
    // Used to manage associated token accounts
    // ie where a wallet holds a specific type of token
    pub associated_token_program: Program<'info, AssociatedToken>,

    // Each side can use either the classic token program or the newer token
    // extensions program, so an SPL Token can be swapped for a Token-2022 token
    pub token_program_a: Interface<'info, TokenInterface>,
    pub token_program_b: Interface<'info, TokenInterface>,

    // Used to create accounts
    pub system_program: Program<'info, System>,

    #[account(mut)]
    pub maker: Signer<'info>,

    #[account(mint::token_program = token_program_a)]
    pub token_mint_a: Box<InterfaceAccount<'info, Mint>>,

    #[account(mint::token_program = token_program_b)]
    pub token_mint_b: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        associated_token::mint = token_mint_a,
        associated_token::authority = maker,
        associated_token::token_program = token_program_a
    )]
    pub maker_token_account_a: Box<InterfaceAccount<'info, TokenAccount>>,

    // The maker is part of the seeds, so one maker can't squat or collide
    // with another maker's offer IDs
    #[account(
        init,
        payer = maker,
        space = Offer::DISCRIMINATOR.len() + Offer::INIT_SPACE,
        seeds = [OFFER_SEED, maker.key().as_ref(), id.to_le_bytes().as_ref()],
        bump
    )]
    pub offer: Box<Account<'info, Offer>>,

    #[account(
        init,
        payer = maker,
        associated_token::mint = token_mint_a,
        associated_token::authority = offer,
        associated_token::token_program = token_program_a
    )]
    pub vault: Box<InterfaceAccount<'info, TokenAccount>>,
}

// Handle the make offer instruction by:
// 1. Moving the tokens from the maker's ATA to the vault
// 2. Saving the details of the offer to the offer account
pub fn make_offer(
    context: Context<MakeOffer>,
    id: u64,
    token_a_offered_amount: u64,
    token_b_wanted_amount: u64,
    allow_partial_fills: bool,
    expires_at: i64,
) -> Result<()> {
    // Validate amounts
    require!(token_a_offered_amount > 0, ErrorCode::InvalidAmount);
    require!(token_b_wanted_amount > 0, ErrorCode::InvalidAmount);

    // Validate token mints are different
    require!(
        context.accounts.token_mint_a.key() != context.accounts.token_mint_b.key(),
        ErrorCode::InvalidTokenMint
    );

    let now = Clock::get()?.unix_timestamp;
    require!(
        expires_at == NO_EXPIRY || expires_at > now,
        ErrorCode::InvalidExpiry
    );

    // Checked up front: a failed token CPI aborts the transaction with the
    // token program's error, so it can't be mapped to ours afterwards
    require!(
        context.accounts.maker_token_account_a.amount >= token_a_offered_amount,
        ErrorCode::InsufficientMakerBalance
    );

    // Move the tokens from the maker's ATA to the vault
    transfer_tokens(
        &context.accounts.maker_token_account_a,
        &context.accounts.vault,
        &token_a_offered_amount,
        &context.accounts.token_mint_a,
        &context.accounts.maker.to_account_info(),
        &context.accounts.token_program_a,
        None,
    )?;

    // A Token-2022 transfer fee may have been withheld on the way in, so the
    // offer is for what actually reached the vault
    context.accounts.vault.reload()?;
    let deposited = context.accounts.vault.amount;
    require!(deposited > 0, ErrorCode::InvalidAmount);

    // Save the details of the offer to the offer account
    context.accounts.offer.set_inner(Offer {
        id,
        maker: context.accounts.maker.key(),
        token_mint_a: context.accounts.token_mint_a.key(),
        token_mint_b: context.accounts.token_mint_b.key(),
        token_a_offered_amount: deposited,
        token_b_wanted_amount,
        token_a_remaining: deposited,
        token_b_received: 0,
        allow_partial_fills,
        expires_at,
        bump: context.bumps.offer,
    });

    emit!(OfferMade {
        offer: context.accounts.offer.key(),
        maker: context.accounts.maker.key(),
        token_mint_a: context.accounts.token_mint_a.key(),
        token_mint_b: context.accounts.token_mint_b.key(),
        token_a_offered_amount: deposited,
        token_b_wanted_amount,
        allow_partial_fills,
        expires_at,
    });
    Ok(())
}
