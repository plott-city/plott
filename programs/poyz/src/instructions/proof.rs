//! On-chain execution proof for a rebalance.
//!
//! # What is committed, and what that makes verifiable
//!
//! Two hashes, with different trust properties.
//!
//! # What the program computes versus what it accepts
//!
//! `collateral_notional` and `delta_bps_after` arrive as keeper arguments, but
//! they are **not** what gets stored. The program recomputes both from its own
//! state -- `Config::total_collateral` valued at the Pyth price posted in this
//! same transaction -- enforces the band on *its* number, and rejects the call
//! if the keeper's claim disagrees. The record then holds the program's values.
//!
//! `hedged_notional` remains an attestation: this program cannot see the venue
//! account, so the short leg is the one number it must take on trust. That is
//! exactly why it is bonded and slashable, and why the venue payload is hashed.
//! Everything the program *can* derive, it derives.
//!
//! `venues_hash` is supplied by the keeper. It is `sha256` over the Borsh
//! encoding of the venue-side execution payload, in this exact field order (the
//! canonical encoder lives in `packages/delta-keeper`, and `packages/sdk-ts`
//! ships the verifier):
//!
//! ```text
//! ExecutionPayload {
//!   config:              Pubkey,      // domain separation: this protocol
//!   sequence:            u64,         // domain separation: this rebalance
//!   keeper:              Pubkey,      // who is accountable
//!   venue_id:            u8,          // state::VENUE_*; see the note below
//!   venue_subaccount:    Pubkey,      // the venue account that must show these fills
//!   delta_bps_before:    i32,
//!   delta_bps_after:     i32,
//!   collateral_notional: u64,
//!   hedged_notional:     u64,
//!   oracle_price:        i64,
//!   oracle_conf:         u64,
//!   oracle_expo:         i32,
//!   oracle_publish_time: i64,
//!   fills: Vec<Fill { order_id: u64, price: i64, base_amount: i64, ts: i64 }>,
//! }
//! ```
//!
//! `this_hash` is computed **by the program**, never supplied:
//!
//! ```text
//! this_hash = sha256(
//!     prev_hash || config || sequence || slot || oracle_price || oracle_conf ||
//!     oracle_expo || oracle_publish_time || collateral_notional ||
//!     hedged_notional || delta_bps_before || delta_bps_after || venue_id ||
//!     venues_hash || keeper )
//! ```
//!
//! and becomes `Config::last_proof_hash`, so the proofs form a chain. Because
//! the chain head lives in the config and every link is computed on-chain from
//! values the program itself just validated, a keeper cannot rewrite history:
//! altering any past field changes every subsequent `this_hash`, and the head
//! on-chain would no longer match.
//!
//! Storing digests keeps the account at a fixed 232 bytes no matter how many
//! fills a rebalance took, while still committing to every one of them. The
//! summary fields stored alongside are the same values that go into the hash,
//! so they cannot be restated later without producing a different digest.
//!
//! # Venue ids in a proof
//!
//! `venue_id` is 1-based on purpose. Id 0 means "unset", not "the primary
//! venue", so a caller that forgets to populate the field is rejected instead
//! of having its proof silently attributed to Velocity -- a misattribution no
//! type check would catch, since any `u8` is a syntactically valid id.
//!
//! The slots also carry history:
//!
//!   * **1 = Velocity.** This slot was Drift, exploited in 2026-04 and
//!     rebranded to Velocity on 2026-07-01 (`drift.trade` no longer resolves).
//!     Same venue, same slot, so proofs committed before the rebrand keep their
//!     meaning. Off-chain, `drift` must be a rename alias of `velocity`, never
//!     a second venue.
//!   * **2 = Jupiter Perps.** This slot was Zeta, which wound down in 2025-05
//!     (Mango v4 likewise). Reusing the slot is safe because no Zeta proof can
//!     exist on a program that never had a Zeta integration.
//!   * **3, 4 = Adrena, Flash Trade.** Reserved, and refused until
//!     `Config::venue_flags` enables them.
//!   * **255 = simulated.** Never committable. A proof records something that
//!     happened.
//!
//! Both checks are applied: the id must be in range *and* its flag bit must be
//! set. Range alone would let a proof name a venue the protocol has not
//! integrated.
//!
//! An observer with the keeper's venue subaccount -- public data on Velocity --
//! can pull the fills for the proof's slot window, re-encode the payload, and
//! compare against `venues_hash`. Three outcomes:
//!
//!   * digests match  -> the keeper's claimed execution is the execution the
//!     venue actually recorded;
//!   * digests differ -> the keeper published a claim the venue does not
//!     support. That is the evidence bundle behind `keeper_slash` with reason
//!     `SLASH_REASON_FALSE_PROOF`;
//!   * no proof at all for a slot range where the delta was out of band -> the
//!     keeper did not rebalance. Detectable from the gap in `sequence`, and
//!     slashable as `SLASH_REASON_LIVENESS`.
//!
//! What this does NOT prove: that the venue is solvent, that the fills were
//! good prices, or that the reported delta was computed correctly from
//! positions this program cannot see. Those are venue and keeper risk,
//! disclosed in `docs/risk-spec.md`. The proof is an accountability primitive,
//! not a solvency proof, and this program does not claim otherwise.
//!
//! # Replay and reordering
//!
//! Three independent guards:
//!   * `sequence` must equal `Config::rebalance_count`, which then increments,
//!     so proofs form a gapless chain;
//!   * a replayed sequence collides with an existing PDA and fails at account
//!     creation, before the handler runs;
//!   * `Clock::slot` must be strictly greater than `Config::last_proof_slot`,
//!     so two proofs can never share a slot.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::hashv;

use crate::errors::PoyzError;
use crate::events::RebalanceProofCommitted;
use crate::math::{collateral_to_notional, delta_bps, mul_bps_ceil};
use crate::oracle::load_price;
use crate::state::*;

/// Sanity bound for reported delta deviations: +/- 100x notional. Anything
/// beyond it is a units bug in the keeper, not a market condition.
const DELTA_BPS_LIMIT: i32 = 1_000_000;

#[derive(Accounts)]
#[instruction(sequence: u64)]
pub struct CommitRebalanceProof<'info> {
    #[account(mut)]
    pub keeper: Signer<'info>,

    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = oracle @ PoyzError::OracleAccountMismatch,
    )]
    pub config: Box<Account<'info, Config>>,

    #[account(
        mut,
        seeds = [KEEPER_SEED, keeper.key().as_ref()],
        bump = keeper_account.bump,
        has_one = keeper @ PoyzError::Unauthorized,
    )]
    pub keeper_account: Box<Account<'info, Keeper>>,

    #[account(
        init,
        payer = keeper,
        space = 8 + RebalanceProof::LEN,
        seeds = [REBALANCE_PROOF_SEED, &sequence.to_le_bytes()],
        bump
    )]
    pub proof: Box<Account<'info, RebalanceProof>>,

    /// CHECK: authenticated in `oracle::load_price` (owner, discriminator,
    /// verification level, feed id, staleness, confidence).
    pub oracle: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[allow(clippy::too_many_arguments)]
pub fn commit_rebalance_proof(
    ctx: Context<CommitRebalanceProof>,
    sequence: u64,
    venues_hash: [u8; 32],
    venue_id: u8,
    delta_bps_before: i32,
    delta_bps_after: i32,
    hedged_notional: u64,
    collateral_notional: u64,
) -> Result<()> {
    let config = &ctx.accounts.config;

    // Deliberately callable while the protocol is paused. A pause stops
    // issuance; the book still exists and still drifts, and the keeper must
    // keep rebalancing and attesting through the incident. Blocking proofs
    // during a pause would blind exactly the period that needs the record.

    // Only a bonded keeper may make a claim the chain cannot check. Both the
    // flag and the live balance are tested: the flag can lag a parameter
    // change that raised `min_keeper_bond`.
    require!(
        ctx.accounts.keeper_account.active,
        PoyzError::KeeperInactive
    );
    require!(
        ctx.accounts.keeper_account.bonded >= config.min_keeper_bond,
        PoyzError::InsufficientBond
    );

    require!(venues_hash != [0u8; 32], PoyzError::EmptyProofHash);
    require!(config.venue_enabled(venue_id), PoyzError::VenueNotEnabled);
    require!(
        sequence == config.rebalance_count,
        PoyzError::ProofSequenceMismatch
    );
    require!(collateral_notional > 0, PoyzError::ZeroAmount);
    require!(
        delta_bps_before.abs() <= DELTA_BPS_LIMIT && delta_bps_after.abs() <= DELTA_BPS_LIMIT,
        PoyzError::DeltaOutOfRange
    );

    let clock = Clock::get()?;
    require!(
        clock.slot > config.last_proof_slot,
        PoyzError::ProofSlotNotMonotonic
    );

    // A proof committed against a stale or wide-confidence oracle asserts a
    // delta that was computed from a price nobody can defend.
    let price = load_price(
        &ctx.accounts.oracle.to_account_info(),
        &config.oracle,
        &config.feed_id,
        config.max_price_age_sec,
        config.max_conf_bps,
        clock.unix_timestamp,
    )?;

    // Recompute the collateral leg from on-chain state at the price posted in
    // this transaction. This is the half of the delta the program can derive
    // for itself, so it does.
    let book_notional = collateral_to_notional(
        config.total_collateral,
        price.price,
        price.expo,
        config.collateral_decimals,
        config.synthetic_decimals,
    )?;
    // An empty book has no delta, and therefore nothing to prove about.
    require!(book_notional > 0, PoyzError::ZeroAmount);

    // The keeper's claim has to agree with the program's valuation. A relative
    // tolerance rather than equality, because `total_collateral` can move
    // between the keeper building the transaction and it landing (a concurrent
    // mint confirm); the band is the same one used for hedge fills.
    let tolerance = mul_bps_ceil(book_notional, config.max_hedge_slippage_bps)?;
    require!(
        collateral_notional.abs_diff(book_notional) <= tolerance,
        PoyzError::ProofCollateralMismatch
    );

    // The invariant, enforced on the program's own number rather than on the
    // keeper's: a proof for an unbalanced book cannot exist. Note this is the
    // *inner* exit target, not the outer band that triggers a rebalance --
    // clearing the trigger is not the same as finishing the job, and a keeper
    // allowed to stop at the band would leave the book permanently at the edge
    // of tolerance.
    let delta_after_onchain = delta_bps(book_notional, hedged_notional)?;
    require!(
        u64::from(delta_after_onchain.unsigned_abs()) <= u64::from(config.delta_exit_bps),
        PoyzError::DeltaThresholdExceeded
    );

    // ... and the keeper's reported delta has to match what the program
    // derived. A keeper whose own accounting disagrees with the chain is
    // running on a book it has mis-modelled, which is worth catching even when
    // the chain's own number happens to be inside the band.
    let claim_gap = i64::from(delta_bps_after)
        .checked_sub(i64::from(delta_after_onchain))
        .ok_or(PoyzError::MathOverflow)?
        .abs();
    require!(
        claim_gap <= i64::from(config.delta_exit_bps),
        PoyzError::ProofDeltaMismatch
    );

    // From here on the program's values are the record.
    let collateral_notional = book_notional;
    let delta_bps_after = delta_after_onchain;

    let config_key = config.key();
    let keeper_key = ctx.accounts.keeper.key();
    let prev_hash = config.last_proof_hash;

    let this_hash = hashv(&[
        &prev_hash,
        config_key.as_ref(),
        &sequence.to_le_bytes(),
        &clock.slot.to_le_bytes(),
        &price.price.to_le_bytes(),
        &price.conf.to_le_bytes(),
        &price.expo.to_le_bytes(),
        &price.publish_time.to_le_bytes(),
        &collateral_notional.to_le_bytes(),
        &hedged_notional.to_le_bytes(),
        &delta_bps_before.to_le_bytes(),
        &delta_bps_after.to_le_bytes(),
        &[venue_id],
        &venues_hash,
        keeper_key.as_ref(),
    ])
    .to_bytes();

    let proof = &mut ctx.accounts.proof;
    proof.keeper = keeper_key;
    proof.venues_hash = venues_hash;
    proof.prev_hash = prev_hash;
    proof.this_hash = this_hash;
    proof.sequence = sequence;
    proof.hedged_notional = hedged_notional;
    proof.collateral_notional = collateral_notional;
    proof.oracle_publish_time = price.publish_time;
    proof.oracle_posted_slot = price.posted_slot;
    proof.slot = clock.slot;
    proof.timestamp = clock.unix_timestamp;
    proof.oracle_price = price.price;
    proof.oracle_conf = price.conf;
    proof.delta_bps_before = delta_bps_before;
    proof.delta_bps_after = delta_bps_after;
    proof.oracle_expo = price.expo;
    proof.venue_id = venue_id;
    proof.bump = ctx.bumps.proof;
    proof.reserved = [0u8; 18];
    let proof_key = proof.key();

    let keeper_account = &mut ctx.accounts.keeper_account;
    keeper_account.proofs_committed = keeper_account
        .proofs_committed
        .checked_add(1)
        .ok_or(PoyzError::MathOverflow)?;
    keeper_account.last_proof_at = clock.unix_timestamp;
    keeper_account.last_proof_slot = clock.slot;

    let config = &mut ctx.accounts.config;
    config.rebalance_count = config
        .rebalance_count
        .checked_add(1)
        .ok_or(PoyzError::MathOverflow)?;
    config.last_proof_slot = clock.slot;
    config.last_proof_hash = this_hash;
    config.hedged_notional = hedged_notional;

    emit!(RebalanceProofCommitted {
        config: config_key,
        proof: proof_key,
        keeper: keeper_key,
        sequence,
        venues_hash,
        prev_hash,
        this_hash,
        venue_id,
        delta_bps_before,
        delta_bps_after,
        hedged_notional,
        collateral_notional,
        oracle_price: price.price,
        oracle_conf: price.conf,
        oracle_expo: price.expo,
        oracle_publish_time: price.publish_time,
        oracle_posted_slot: price.posted_slot,
        slot: clock.slot,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
