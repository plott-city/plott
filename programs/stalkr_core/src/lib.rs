use anchor_lang::prelude::*;

mod state;
mod contexts;
mod errors;

use contexts::*;
use errors::StalkrError;

declare_id!("STKRprdWkXJYxLRmec9NzDbDjhKvVCNhma5r74yAGFN");

#[program]
pub mod stalkr_core {
    use super::*;

    pub fn initialize_tracker(ctx: Context<InitializeTracker>, config: TrackerConfig) -> Result<()> {
        let tracker = &mut ctx.accounts.tracker;
        tracker.authority = ctx.accounts.authority.key();
        tracker.whale_count = 0;
        tracker.prediction_count = 0;
        tracker.funding_weight = config.funding_weight;
        tracker.time_weight = config.time_weight;
        tracker.sequence_weight = config.sequence_weight;
        tracker.trigger_weight = config.trigger_weight;
        tracker.confidence_threshold = config.confidence_threshold;
        tracker.bump = ctx.bumps.tracker;
        Ok(())
    }

    pub fn register_whale(
        ctx: Context<RegisterWhale>,
        address: Pubkey,
        tier: u8,
        label: String,
    ) -> Result<()> {
        require!(tier <= 2, StalkrError::InvalidTier);
        require!(label.len() <= 32, StalkrError::LabelTooLong);

        let whale = &mut ctx.accounts.whale;
        whale.address = address;
        whale.tier = tier;
        whale.label = label;
        whale.prediction_count = 0;
        whale.correct_predictions = 0;
        whale.last_active = Clock::get()?.unix_timestamp;
        whale.bump = ctx.bumps.whale;

        let tracker = &mut ctx.accounts.tracker;
        tracker.whale_count = tracker.whale_count.checked_add(1).ok_or(StalkrError::Overflow)?;

        Ok(())
    }

    pub fn record_prediction(
        ctx: Context<RecordPrediction>,
        confidence: u8,
        predicted_action: u8,
        window_hours: u16,
        patterns_matched: u8,
    ) -> Result<()> {
        require!(confidence >= 50 && confidence <= 99, StalkrError::InvalidConfidence);
        require!(predicted_action <= 4, StalkrError::InvalidAction);
        require!(window_hours >= 1 && window_hours <= 168, StalkrError::InvalidWindow);

        let prediction = &mut ctx.accounts.prediction;
        prediction.whale = ctx.accounts.whale.key();
        prediction.authority = ctx.accounts.authority.key();
        prediction.confidence = confidence;
        prediction.predicted_action = predicted_action;
        prediction.window_hours = window_hours;
        prediction.patterns_matched = patterns_matched;
        prediction.created_at = Clock::get()?.unix_timestamp;
        prediction.resolved = false;
        prediction.was_correct = false;
        prediction.bump = ctx.bumps.prediction;

        let whale = &mut ctx.accounts.whale;
        whale.prediction_count = whale.prediction_count.checked_add(1).ok_or(StalkrError::Overflow)?;

        let tracker = &mut ctx.accounts.tracker;
        tracker.prediction_count = tracker.prediction_count.checked_add(1).ok_or(StalkrError::Overflow)?;

        Ok(())
    }

    pub fn resolve_prediction(
        ctx: Context<ResolvePrediction>,
        was_correct: bool,
    ) -> Result<()> {
        let prediction = &mut ctx.accounts.prediction;
        require!(!prediction.resolved, StalkrError::AlreadyResolved);

        prediction.resolved = true;
        prediction.was_correct = was_correct;
        prediction.resolved_at = Clock::get()?.unix_timestamp;

        if was_correct {
            let whale = &mut ctx.accounts.whale;
            whale.correct_predictions = whale
                .correct_predictions
                .checked_add(1)
                .ok_or(StalkrError::Overflow)?;
        }

        Ok(())
    }
}

#[derive(AnchorDeserialize, AnchorSerialize)]
pub struct TrackerConfig {
    pub funding_weight: u8,
    pub time_weight: u8,
    pub sequence_weight: u8,
    pub trigger_weight: u8,
    pub confidence_threshold: u8,
}

// correct response deserialization for empty arrays
