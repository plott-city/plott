use anchor_lang::prelude::*;
use crate::state::*;

#[derive(Accounts)]
pub struct InitializeTracker<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + Tracker::LEN,
        seeds = [b"tracker"],
        bump,
    )]
    pub tracker: Account<'info, Tracker>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(address: Pubkey)]
pub struct RegisterWhale<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + Whale::LEN,
        seeds = [b"whale", address.as_ref()],
        bump,
    )]
    pub whale: Account<'info, Whale>,
    #[account(
        mut,
        seeds = [b"tracker"],
        bump = tracker.bump,
        has_one = authority,
    )]
    pub tracker: Account<'info, Tracker>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RecordPrediction<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + Prediction::LEN,
        seeds = [
            b"prediction",
            whale.key().as_ref(),
            &tracker.prediction_count.to_le_bytes(),
        ],
        bump,
    )]
    pub prediction: Account<'info, Prediction>,
    #[account(
        mut,
        seeds = [b"whale", whale.address.as_ref()],
        bump = whale.bump,
    )]
    pub whale: Account<'info, Whale>,
    #[account(
        mut,
        seeds = [b"tracker"],
        bump = tracker.bump,
        has_one = authority,
    )]
    pub tracker: Account<'info, Tracker>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ResolvePrediction<'info> {
    #[account(
        mut,
        has_one = authority,
    )]
    pub prediction: Account<'info, Prediction>,
    #[account(
        mut,
        seeds = [b"whale", whale.address.as_ref()],
        bump = whale.bump,
    )]
    pub whale: Account<'info, Whale>,
    pub authority: Signer<'info>,
}

// add volatility index calculation

// add missing account close constraint
