use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Insufficient token balance in maker's account")]
    InsufficientMakerBalance,

    #[msg("Insufficient token balance in taker's account")]
    InsufficientTakerBalance,

    #[msg("Invalid token mint - must be different from offered token")]
    InvalidTokenMint,

    #[msg("Amount must be greater than zero")]
    InvalidAmount,

    #[msg("The offer has expired")]
    OfferExpired,

    #[msg("Expiry must be in the future, or 0 for no expiry")]
    InvalidExpiry,

    #[msg("This offer must be filled in full")]
    PartialFillNotAllowed,

    #[msg("Requested more tokens than the offer has left")]
    AmountExceedsRemaining,

    #[msg("The price in token B is higher than the taker's maximum")]
    SlippageExceeded,

    #[msg("Only the maker can refund an offer before it expires")]
    RefundNotAllowed,
}
