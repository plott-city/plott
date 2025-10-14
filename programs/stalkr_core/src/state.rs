use anchor_lang::prelude::*;

#[account]
pub struct Tracker {
    pub authority: Pubkey,
    pub whale_count: u64,
    pub prediction_count: u64,
    pub funding_weight: u8,
    pub time_weight: u8,
    pub sequence_weight: u8,
    pub trigger_weight: u8,
    pub confidence_threshold: u8,
    pub bump: u8,
    pub _padding: [u8; 6],
}

impl Tracker {
    pub const LEN: usize = 32 + 8 + 8 + 1 + 1 + 1 + 1 + 1 + 1 + 6;
}

#[account]
pub struct Whale {
    pub address: Pubkey,
    pub tier: u8,
    pub label: String,
    pub prediction_count: u64,
    pub correct_predictions: u64,
    pub last_active: i64,
    pub bump: u8,
    pub _padding: [u8; 5],
}

impl Whale {
    pub const LEN: usize = 32 + 1 + (4 + 32) + 8 + 8 + 8 + 1 + 5;
}

#[account]
pub struct Prediction {
    pub whale: Pubkey,
    pub authority: Pubkey,
    pub confidence: u8,
    pub predicted_action: u8,
    pub window_hours: u16,
    pub patterns_matched: u8,
    pub created_at: i64,
    pub resolved: bool,
    pub was_correct: bool,
    pub resolved_at: i64,
    pub bump: u8,
    pub _padding: [u8; 4],
}

impl Prediction {
    pub const LEN: usize = 32 + 32 + 1 + 1 + 2 + 1 + 8 + 1 + 1 + 8 + 1 + 4;
}
