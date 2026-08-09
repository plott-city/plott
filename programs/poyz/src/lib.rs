//! Poyz -- delta-neutral synthetic dollar on Solana.
//!
//! # What this program is
//!
//! Collateral accounting, issuance and redemption of the synthetic dollar
//! (pUSD), Delta Keeper bonding and slashing, on-chain execution proofs for
//! every rebalance, funding distribution to stakers, and an insurance buffer.
//!
//! # What this program is not
//!
//! It is not a perpetuals exchange and it never opens a position. The hedge
//! legs are opened on Velocity by `packages/hedge-router`, off-chain, and
//! are attested here: `mint_confirm` and `redeem_confirm` require a bonded
//! keeper to commit a hash of the venue-side execution, and
//! `commit_rebalance_proof` records the ongoing rebalances. What the chain
//! guarantees is that issuance is bound to an attested hedge and that a false
//! attestation is slashable -- not that a venue is solvent. That distinction is
//! stated here, in `docs/risk-spec.md`, and on the website, because the
//! failure mode of this product class is a protocol that quietly implies more
//! than it can prove.
//!
//! # Module map
//!
//! | module         | contents                                             |
//! |----------------|------------------------------------------------------|
//! | `state`        | accounts, PDA seeds, parameter bounds                |
//! | `errors`       | every named failure                                  |
//! | `events`       | one event per state transition                       |
//! | `math`         | integer fixed-point, explicit rounding direction     |
//! | `oracle`       | Pyth `PriceUpdateV2` adapter and its gates           |
//! | `instructions` | one submodule per instruction group                  |
//!
//! # Deployment
//!
//! No deployment is automated anywhere in this package. See the README section
//! "Deployment (requires explicit user approval)".

use anchor_lang::prelude::*;

pub mod errors;
pub mod events;
pub mod instructions;
pub mod math;
pub mod oracle;
pub mod state;

use instructions::*;

declare_id!("9hefehGRVBDE2A9kby8oQnRvEF5yK42px2ssfsQjchzU");

#[program]
pub mod poyz {
    use super::*;

    // -- protocol configuration -------------------------------------------

    /// Create the protocol config PDA. The protocol starts paused; the vault
    /// token accounts do not exist yet.
    pub fn initialize(ctx: Context<Initialize>, params: InitializeParams) -> Result<()> {
        instructions::admin::initialize(ctx, params)
    }

    /// Update protocol parameters. Authority only. Every field is optional and
    /// the whole resulting config is re-validated against the bounds in
    /// `state`, which the authority cannot exceed.
    pub fn set_params(ctx: Context<AdminOnly>, params: UpdateParams) -> Result<()> {
        instructions::admin::set_params(ctx, params)
    }

    /// Set the mint and redeem circuit breakers independently. The authority
    /// may set either flag; the guardian may only move them toward paused.
    /// Unpausing requires all vault token accounts to exist.
    pub fn set_paused(
        ctx: Context<SetPaused>,
        mint_paused: bool,
        redeem_paused: bool,
    ) -> Result<()> {
        instructions::admin::set_paused(ctx, mint_paused, redeem_paused)
    }

    /// Replace the pause-only guardian key. Authority only.
    pub fn set_guardian(ctx: Context<AdminOnly>, guardian: Pubkey) -> Result<()> {
        instructions::admin::set_guardian(ctx, guardian)
    }

    /// Publish the hedge venue's net carry and hedgeable capacity. Feeds the
    /// two issuance gates (carry floor, capacity ceiling) and the negative
    /// funding clock. Both gates fail closed once this goes stale.
    pub fn report_venue_state(
        ctx: Context<AdminOnly>,
        venue_id: u8,
        net_carry_bps: i32,
        capacity_notional: u64,
    ) -> Result<()> {
        instructions::admin::report_venue_state(ctx, venue_id, net_carry_bps, capacity_notional)
    }

    /// Repoint the protocol at a different Pyth price update account. The new
    /// account is authenticated before it is stored.
    pub fn set_oracle(ctx: Context<SetOracle>, feed_id: [u8; 32]) -> Result<()> {
        instructions::admin::set_oracle(ctx, feed_id)
    }

    /// Propose a new authority. Step one of two.
    pub fn transfer_authority(ctx: Context<AdminOnly>, new_authority: Pubkey) -> Result<()> {
        instructions::admin::transfer_authority(ctx, new_authority)
    }

    /// Accept a proposed authority. Step two of two, signed by the incoming
    /// authority so an unusable address can never take over.
    pub fn accept_authority(ctx: Context<AcceptAuthority>) -> Result<()> {
        instructions::admin::accept_authority(ctx)
    }

    // -- vault creation ----------------------------------------------------

    /// Create the collateral vault token account.
    pub fn init_collateral_vault(ctx: Context<InitCollateralVault>) -> Result<()> {
        instructions::vaults::init_collateral_vault(ctx)
    }

    /// Create the live-bond and slashed-bond ($POYZ) vaults.
    pub fn init_bond_vaults(ctx: Context<InitBondVaults>) -> Result<()> {
        instructions::vaults::init_bond_vaults(ctx)
    }

    /// Create the funding vault and the insurance buffer vault.
    pub fn init_funding_vaults(ctx: Context<InitFundingVaults>) -> Result<()> {
        instructions::vaults::init_funding_vaults(ctx)
    }

    /// Create the stake vault and the redeem escrow.
    pub fn init_stake_vaults(ctx: Context<InitStakeVaults>) -> Result<()> {
        instructions::vaults::init_stake_vaults(ctx)
    }

    // -- keeper ------------------------------------------------------------

    /// Register as a Delta Keeper and post the initial $POYZ bond.
    pub fn keeper_register(ctx: Context<KeeperRegister>, bond_amount: u64) -> Result<()> {
        instructions::keeper::keeper_register(ctx, bond_amount)
    }

    /// Top up an existing bond. Re-activates a keeper knocked below the
    /// minimum by a slash.
    pub fn keeper_bond(ctx: Context<KeeperBond>, amount: u64) -> Result<()> {
        instructions::keeper::keeper_bond(ctx, amount)
    }

    /// Withdraw bond. Gated on the unbond cooldown since the keeper's last
    /// committed proof.
    pub fn keeper_unbond(ctx: Context<KeeperUnbond>, amount: u64) -> Result<()> {
        instructions::keeper::keeper_unbond(ctx, amount)
    }

    /// Slash a keeper bond into the insurance buffer's $POYZ vault. Authority
    /// only. `reason_code` must name one of the enumerated faults in
    /// `state::SLASH_REASON_*`, and `evidence_hash` commits to the off-chain
    /// evidence bundle that supports it.
    pub fn keeper_slash(
        ctx: Context<KeeperSlash>,
        amount: u64,
        reason_code: u8,
        evidence_hash: [u8; 32],
    ) -> Result<()> {
        instructions::keeper::keeper_slash(ctx, amount, reason_code, evidence_hash)
    }

    // -- execution proof ---------------------------------------------------

    /// Commit the execution proof for one rebalance. Bonded keepers only,
    /// gapless sequence, strictly increasing slot, fresh oracle, and the
    /// post-rebalance delta inside the inner exit target. The proof chain head
    /// is computed on-chain, never supplied.
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
        instructions::proof::commit_rebalance_proof(
            ctx,
            sequence,
            venues_hash,
            venue_id,
            delta_bps_before,
            delta_bps_after,
            hedged_notional,
            collateral_notional,
        )
    }

    // -- mint --------------------------------------------------------------

    /// Phase one of a mint: escrow collateral and quote a notional. Nothing is
    /// issued here.
    pub fn mint_request(
        ctx: Context<MintRequestCtx>,
        nonce: u64,
        collateral_amount: u64,
        min_synthetic_out: u64,
    ) -> Result<()> {
        instructions::mint::mint_request(ctx, nonce, collateral_amount, min_synthetic_out)
    }

    /// Phase two of a mint: a bonded keeper attests the hedge fill, and the
    /// synthetic dollars are issued against the lower of the two quotes.
    pub fn mint_confirm(
        ctx: Context<MintConfirmCtx>,
        nonce: u64,
        hedge_proof_hash: [u8; 32],
        venue_id: u8,
        filled_notional: u64,
    ) -> Result<()> {
        instructions::mint::mint_confirm(ctx, nonce, hedge_proof_hash, venue_id, filled_notional)
    }

    /// Reclaim escrowed collateral from an expired mint request. User only,
    /// callable while paused.
    pub fn mint_cancel(ctx: Context<MintCancelCtx>, nonce: u64) -> Result<()> {
        instructions::mint::mint_cancel(ctx, nonce)
    }

    // -- redeem ------------------------------------------------------------

    /// Phase one of a redeem: escrow synthetic and quote the collateral.
    pub fn redeem_request(
        ctx: Context<RedeemRequestCtx>,
        nonce: u64,
        synthetic_amount: u64,
        min_collateral_out: u64,
    ) -> Result<()> {
        instructions::redeem::redeem_request(ctx, nonce, synthetic_amount, min_collateral_out)
    }

    /// Phase two of a redeem: a bonded keeper attests the unwind, the escrowed
    /// synthetic is burned and collateral is released.
    pub fn redeem_confirm(
        ctx: Context<RedeemConfirmCtx>,
        nonce: u64,
        unwind_proof_hash: [u8; 32],
        venue_id: u8,
        unwound_notional: u64,
    ) -> Result<()> {
        instructions::redeem::redeem_confirm(
            ctx,
            nonce,
            unwind_proof_hash,
            venue_id,
            unwound_notional,
        )
    }

    /// Reclaim escrowed synthetic from an expired redeem request. User only,
    /// callable while paused.
    pub fn redeem_cancel(ctx: Context<RedeemCancelCtx>, nonce: u64) -> Result<()> {
        instructions::redeem::redeem_cancel(ctx, nonce)
    }

    // -- funding, staking, insurance buffer --------------------------------

    /// Book a funding settlement. Authority only. The carry regime itself is
    /// written by `report_venue_state`, not here.
    pub fn settle_funding(ctx: Context<SettleFunding>, amount: u64) -> Result<()> {
        instructions::funding::settle_funding(ctx, amount)
    }

    /// Stake synthetic dollars to earn funding.
    pub fn stake(ctx: Context<Stake>, amount: u64) -> Result<()> {
        instructions::funding::stake(ctx, amount)
    }

    /// Begin an exit. The amount stops earning immediately and becomes
    /// withdrawable after `unstake_cooldown_sec`.
    pub fn request_unstake(ctx: Context<UnstakeCtx>, amount: u64) -> Result<()> {
        instructions::funding::request_unstake(ctx, amount)
    }

    /// Withdraw the pending unstake once its cooldown has elapsed. Never
    /// gated on a pause.
    pub fn unstake(ctx: Context<UnstakeCtx>) -> Result<()> {
        instructions::funding::unstake(ctx)
    }

    /// Claim accrued funding, pro rata to the staked amount.
    pub fn claim_funding(ctx: Context<ClaimFunding>) -> Result<()> {
        instructions::funding::claim_funding(ctx)
    }

    /// Deposit into the insurance buffer. Permissionless.
    pub fn buffer_deposit(ctx: Context<BufferDeposit>, amount: u64) -> Result<()> {
        instructions::buffer::buffer_deposit(ctx, amount)
    }

    /// Draw the insurance buffer into the funding vault during a sustained
    /// negative funding regime. Authority only; the destination is pinned by
    /// PDA seeds and cannot be chosen by the caller.
    pub fn buffer_withdraw(ctx: Context<BufferWithdraw>, amount: u64) -> Result<()> {
        instructions::buffer::buffer_withdraw(ctx, amount)
    }
}
