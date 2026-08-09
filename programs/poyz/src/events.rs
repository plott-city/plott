//! Every state transition emits an event.
//!
//! The indexer in `apps/service` reconstructs protocol history from these and
//! nothing else, so an event carries the values needed to rebuild the resulting
//! state without a follow-up account read: totals after the change, not just
//! the delta.

use anchor_lang::prelude::*;

// -- lifecycle --------------------------------------------------------------

#[event]
pub struct ProtocolInitialized {
    pub config: Pubkey,
    pub authority: Pubkey,
    pub collateral_mint: Pubkey,
    pub synthetic_mint: Pubkey,
    pub bond_mint: Pubkey,
    pub oracle: Pubkey,
    pub feed_id: [u8; 32],
    pub collateral_ratio_bps: u16,
    pub delta_band_bps: u16,
    pub min_keeper_bond: u64,
}

#[event]
pub struct VaultGroupInitialized {
    pub config: Pubkey,
    /// Bit added to `Config::vault_flags` by this instruction.
    pub flag: u8,
    pub vault_flags: u8,
}

#[event]
pub struct ParamsUpdated {
    pub config: Pubkey,
    pub authority: Pubkey,
    pub max_price_age_sec: u32,
    pub max_conf_bps: u16,
    pub collateral_ratio_bps: u16,
    pub mint_fee_bps: u16,
    pub redeem_fee_bps: u16,
    pub delta_band_bps: u16,
    pub delta_exit_bps: u16,
    pub delta_hard_bps: u16,
    pub max_hedge_slippage_bps: u16,
    pub buffer_share_bps: u16,
    pub buffer_max_draw_bps: u16,
    pub min_keeper_bond: u64,
    pub max_synthetic_supply: u64,
    pub request_ttl_sec: u32,
    pub min_settlement_delay_sec: u32,
    pub unbond_cooldown_sec: u32,
    pub buffer_unlock_delay_sec: u32,
    pub unstake_cooldown_sec: u32,
    pub max_supply_vs_capacity_bps: u16,
    pub max_venue_state_age_sec: u32,
    pub min_net_carry_bps: i32,
    pub max_reportable_capacity_notional: u64,
    pub venue_flags: u8,
}

#[event]
pub struct VenueStateReported {
    pub config: Pubkey,
    /// Who reported. Recorded so a false report has a name attached to it:
    /// this is the evidence trail behind `SLASH_REASON_CARRY_ANOMALY`.
    pub reporter: Pubkey,
    pub reporter_is_authority: bool,
    pub venue_id: u8,
    pub net_carry_bps: i32,
    /// What the reporter claimed, before the admin ceiling was applied.
    pub reported_capacity: u64,
    /// What was actually stored: `min(reported, max_reportable_capacity)`.
    pub capacity_notional: u64,
    pub negative_funding_since: i64,
    pub slot: u64,
    pub timestamp: i64,
}

#[event]
pub struct PauseChanged {
    pub config: Pubkey,
    pub signer: Pubkey,
    pub mint_paused: bool,
    pub redeem_paused: bool,
}

#[event]
pub struct GuardianChanged {
    pub config: Pubkey,
    pub authority: Pubkey,
    pub previous_guardian: Pubkey,
    pub guardian: Pubkey,
}

#[event]
pub struct OracleUpdated {
    pub config: Pubkey,
    pub authority: Pubkey,
    pub previous_oracle: Pubkey,
    pub oracle: Pubkey,
    pub feed_id: [u8; 32],
}

#[event]
pub struct AuthorityTransferProposed {
    pub config: Pubkey,
    pub current_authority: Pubkey,
    pub pending_authority: Pubkey,
}

#[event]
pub struct AuthorityTransferred {
    pub config: Pubkey,
    pub previous_authority: Pubkey,
    pub new_authority: Pubkey,
}

// -- keeper -----------------------------------------------------------------

#[event]
pub struct KeeperRegistered {
    pub config: Pubkey,
    pub keeper: Pubkey,
    pub bonded: u64,
    pub keeper_count: u32,
    pub timestamp: i64,
}

#[event]
pub struct KeeperBonded {
    pub config: Pubkey,
    pub keeper: Pubkey,
    pub added: u64,
    pub bonded: u64,
    pub active: bool,
}

#[event]
pub struct KeeperUnbonded {
    pub config: Pubkey,
    pub keeper: Pubkey,
    pub withdrawn: u64,
    pub bonded: u64,
    pub active: bool,
}

#[event]
pub struct KeeperSlashed {
    pub config: Pubkey,
    pub keeper: Pubkey,
    pub slashed: u64,
    pub bonded: u64,
    pub active: bool,
    pub reason_code: u8,
    /// Hash of the off-chain evidence bundle that justified the slash.
    pub evidence_hash: [u8; 32],
}

// -- execution proof --------------------------------------------------------

/// `collateral_notional` and `delta_bps_after` here are the program's own
/// recomputation, not the keeper's claim. `hedged_notional` is the keeper's
/// attestation -- the program cannot see the venue.
#[event]
pub struct RebalanceProofCommitted {
    pub config: Pubkey,
    pub proof: Pubkey,
    pub keeper: Pubkey,
    pub sequence: u64,
    pub venues_hash: [u8; 32],
    pub prev_hash: [u8; 32],
    pub this_hash: [u8; 32],
    pub venue_id: u8,
    pub delta_bps_before: i32,
    pub delta_bps_after: i32,
    pub hedged_notional: u64,
    pub collateral_notional: u64,
    pub oracle_price: i64,
    pub oracle_conf: u64,
    pub oracle_expo: i32,
    pub oracle_publish_time: i64,
    pub oracle_posted_slot: u64,
    pub slot: u64,
    pub timestamp: i64,
}

// -- mint -------------------------------------------------------------------

#[event]
pub struct MintRequested {
    pub config: Pubkey,
    pub request: Pubkey,
    pub user: Pubkey,
    pub nonce: u64,
    pub collateral_amount: u64,
    pub quoted_notional: u64,
    pub quoted_price: i64,
    pub quoted_expo: i32,
    pub deadline: i64,
}

#[event]
pub struct MintConfirmed {
    pub config: Pubkey,
    pub user: Pubkey,
    pub keeper: Pubkey,
    pub nonce: u64,
    pub collateral_amount: u64,
    pub effective_notional: u64,
    pub synthetic_minted: u64,
    pub fee: u64,
    pub filled_notional: u64,
    pub venue_id: u8,
    /// Hash of the hedge execution payload proving the offsetting short was
    /// opened before these synthetic dollars existed.
    pub hedge_proof_hash: [u8; 32],
    pub total_synthetic: u64,
    pub total_collateral: u64,
    pub hedged_notional: u64,
    pub timestamp: i64,
}

#[event]
pub struct MintCancelled {
    pub config: Pubkey,
    pub user: Pubkey,
    pub nonce: u64,
    pub collateral_returned: u64,
    pub timestamp: i64,
}

// -- redeem -----------------------------------------------------------------

#[event]
pub struct RedeemRequested {
    pub config: Pubkey,
    pub request: Pubkey,
    pub user: Pubkey,
    pub nonce: u64,
    pub synthetic_amount: u64,
    pub quoted_collateral: u64,
    pub quoted_price: i64,
    pub quoted_expo: i32,
    pub deadline: i64,
}

#[event]
pub struct RedeemConfirmed {
    pub config: Pubkey,
    pub user: Pubkey,
    pub keeper: Pubkey,
    pub nonce: u64,
    pub synthetic_burned: u64,
    pub collateral_returned: u64,
    pub fee: u64,
    pub unwound_notional: u64,
    pub venue_id: u8,
    pub unwind_proof_hash: [u8; 32],
    pub total_synthetic: u64,
    pub total_collateral: u64,
    pub hedged_notional: u64,
    pub timestamp: i64,
}

#[event]
pub struct RedeemCancelled {
    pub config: Pubkey,
    pub user: Pubkey,
    pub nonce: u64,
    pub synthetic_returned: u64,
    pub timestamp: i64,
}

// -- funding / staking / buffer ---------------------------------------------

#[event]
pub struct FundingSettled {
    pub config: Pubkey,
    pub authority: Pubkey,
    pub amount: u64,
    pub to_stakers: u64,
    pub to_buffer: u64,
    /// Carry regime in force at settlement, as last reported by
    /// `report_venue_state`. Carried in the event so an indexer can attribute a
    /// settlement to a regime without a second lookup.
    pub net_carry_bps: i32,
    pub acc_funding_per_share: u128,
    pub total_staked: u64,
    pub negative_funding_since: i64,
    pub timestamp: i64,
}

#[event]
pub struct Staked {
    pub config: Pubkey,
    pub owner: Pubkey,
    pub amount: u64,
    pub position_amount: u64,
    pub total_staked: u64,
}

#[event]
pub struct UnstakeRequested {
    pub config: Pubkey,
    pub owner: Pubkey,
    pub amount: u64,
    pub position_amount: u64,
    pub pending_unstake: u64,
    pub cooldown_end: i64,
    pub total_staked: u64,
}

#[event]
pub struct Unstaked {
    pub config: Pubkey,
    pub owner: Pubkey,
    pub amount: u64,
    pub position_amount: u64,
    pub total_staked: u64,
}

#[event]
pub struct FundingClaimed {
    pub config: Pubkey,
    pub owner: Pubkey,
    pub amount: u64,
    pub claimed_total: u64,
    pub staker_funding_balance: u64,
}

#[event]
pub struct BufferDeposited {
    pub config: Pubkey,
    pub depositor: Pubkey,
    pub amount: u64,
    pub buffer_balance: u64,
}

#[event]
pub struct BufferWithdrawn {
    pub config: Pubkey,
    pub authority: Pubkey,
    pub amount: u64,
    pub buffer_balance: u64,
    pub acc_funding_per_share: u128,
    pub negative_funding_since: i64,
    pub timestamp: i64,
}
