use anchor_lang::prelude::*;

#[error_code]
pub enum StalkrError {
    #[msg("Invalid whale tier value")]
    InvalidTier,
    #[msg("Label exceeds maximum length of 32 characters")]
    LabelTooLong,
    #[msg("Confidence must be between 50 and 99")]
    InvalidConfidence,
    #[msg("Invalid predicted action type")]
    InvalidAction,
    #[msg("Window hours must be between 1 and 168")]
    InvalidWindow,
    #[msg("Prediction has already been resolved")]
    AlreadyResolved,
    #[msg("Arithmetic overflow")]
    Overflow,
}

// separate pattern types into own module [2.11]
