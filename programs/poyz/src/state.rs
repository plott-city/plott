//! On-chain accounts.
//!
//! Every account carries an explicit `LEN` and a `reserved` tail. The tail is
//! upgrade headroom: Anchor account layouts can grow into trailing reserved
//! bytes without a migration, but they can never shrink or reorder, so the
//! space is claimed up front while it is free.
//!
//! Sizes are written out field by field. `space = 8 + T::LEN`, where 8 is the
//! Anchor discriminator. `tests::declared_lengths_match_the_structs` asserts
//! the arithmetic, because a `LEN` that disagrees with the struct silently
//! corrupts every account written after the first upgrade.
//!
//! Seed names and the `Config` field set follow `docs/architecture.md` section
//! 5. Deviations from that document are listed in the package README under
//! "Deviations from docs/architecture.md", each with its reason.

use anchor_lang::prelude::*;

// ---------------------------------------------------------------------------
// PDA seeds
//
// Every seed is a distinct byte string. Numeric seeds are always little-endian
// (`to_le_bytes`) -- the on-chain, SDK and test derivations must agree byte for
// byte, and a big-endian slip is the PDA-mismatch bug in
// `references/solana/anchor-lessons.md`.
// ---------------------------------------------------------------------------

pub const CONFIG_SEED: &[u8] = b"config";
pub const COLLATERAL_VAULT_SEED: &[u8] = b"collateral_vault";
pub const BOND_VAULT_SEED: &[u8] = b"bond_vault";
pub const BUFFER_BOND_VAULT_SEED: &[u8] = b"buffer_bond_vault";
pub const FUNDING_VAULT_SEED: &[u8] = b"funding_vault";
pub const BUFFER_VAULT_SEED: &[u8] = b"buffer_vault";
pub const STAKE_VAULT_SEED: &[u8] = b"stake_vault";
pub const REDEEM_ESCROW_SEED: &[u8] = b"redeem_escrow";
pub const KEEPER_SEED: &[u8] = b"keeper";
pub const MINT_REQUEST_SEED: &[u8] = b"mint_request";
pub const REDEEM_REQUEST_SEED: &[u8] = b"redeem_request";
pub const REBALANCE_PROOF_SEED: &[u8] = b"proof";
pub const STAKE_POSITION_SEED: &[u8] = b"stake";

// ---------------------------------------------------------------------------
// Bounds enforced in code rather than by policy
// ---------------------------------------------------------------------------

/// A delta band wider than 20 % is not delta-neutral in any useful sense.
pub const MAX_DELTA_BAND_BPS: u16 = 2_000;
/// One hour. A price older than this is not a price, it is a memory.
pub const MAX_PRICE_AGE_SEC_LIMIT: u32 = 3_600;
/// Fees are capped in code so a compromised authority cannot set a 100 % fee.
pub const MAX_FEE_BPS: u16 = 500;
/// Collateral ratio can never be set below 1.00x.
pub const MIN_COLLATERAL_RATIO_BPS: u16 = 10_000;
/// ... nor above 5.00x, which would make the product pointless.
pub const MAX_COLLATERAL_RATIO_BPS: u16 = 50_000;
/// SPL mints in scope are 0..=9 decimals.
pub const MAX_MINT_DECIMALS: u8 = 9;

// Bit flags for `Config::vault_flags`.
pub const VAULT_FLAG_COLLATERAL: u8 = 1 << 0;
pub const VAULT_FLAG_BOND: u8 = 1 << 1;
pub const VAULT_FLAG_FUNDING: u8 = 1 << 2;
pub const VAULT_FLAG_STAKE: u8 = 1 << 3;
pub const VAULT_FLAGS_ALL: u8 =
    VAULT_FLAG_COLLATERAL | VAULT_FLAG_BOND | VAULT_FLAG_FUNDING | VAULT_FLAG_STAKE;

// ---------------------------------------------------------------------------
// Venue registry
//
// `venue_id: u8` is a cross-package contract. `packages/hedge-router`,
// `packages/delta-keeper`, `packages/sdk-ts` and `apps/service` all decode this
// byte, and an on-chain execution proof that names the wrong venue points an
// auditor at the wrong trade history. The mapping is fixed in `_DIRECTION.md`
// section 8-1 and mirrored here; this program is the definition site.
//
// Drift is gone: the 2026-04 exploit was followed by the 2026-07-01 Velocity
// rebrand, and `drift.trade` no longer resolves. Zeta and Mango v4 are wound
// down and are not integration targets.
// ---------------------------------------------------------------------------

/// Not a venue. Reserved so that a zeroed byte -- an uninitialised field, a
/// zeroed account, a struct built with `..Default::default()` -- can never be
/// mistaken for a real venue.
///
/// This is the whole reason the mapping is 1-based. With `0 = velocity`, any
/// path that forgot to set `venue_id` would silently attribute its execution
/// proof to the primary venue, and nothing in the type system or the tests
/// would object: the value is a valid `u8` either way. Making 0 invalid turns
/// that class of bug into an instruction-level rejection.
pub const VENUE_NONE: u8 = 0;
/// Velocity. The primary hedge venue.
///
/// Slot 1 was Drift, which was exploited in 2026-04 and rebranded to Velocity
/// on 2026-07-01; `drift.trade` no longer resolves. Keeping Velocity in the
/// same slot preserves the meaning of every proof committed before the
/// rebrand -- the venue is the same venue. Off-chain code must therefore treat
/// `drift` as a rename alias of `velocity`, not as a separate venue.
pub const VENUE_VELOCITY: u8 = 1;
/// Jupiter Perps. Borrow-fee-paying, so it carries a different cost model.
///
/// Slot 2 was Zeta, which wound down in 2025-05 (Mango v4 likewise). The slot
/// is reused rather than retired because no Zeta proof can exist on a program
/// that never had a Zeta integration.
pub const VENUE_JUPITER_PERPS: u8 = 2;
/// Adrena. Reserved, not yet integrated.
pub const VENUE_ADRENA: u8 = 3;
/// Flash Trade. Reserved, not yet integrated.
pub const VENUE_FLASH_TRADE: u8 = 4;
/// Highest assignable venue id.
pub const VENUE_ID_MAX: u8 = VENUE_FLASH_TRADE;
/// Off-chain simulation. Never committable: a proof records something that
/// happened, and a simulated fill did not.
pub const VENUE_SIMULATED: u8 = 255;

/// Bit `n` of `Config::venue_flags` enables venue id `n`. Bit 0 is permanently
/// unused, because venue id 0 is not a venue.
///
/// Total by construction: ids outside the assignable range map to no bit at
/// all, so a shift can never overflow and an out-of-range id can never be
/// enabled by any flag value, including `0xFF`.
pub const fn venue_bit(venue_id: u8) -> u8 {
    if venue_id == VENUE_NONE || venue_id > VENUE_ID_MAX {
        0
    } else {
        1u8 << venue_id
    }
}

/// Every bit that may legally be set in `Config::venue_flags`: ids 1..=4.
pub const VENUE_FLAGS_MASK: u8 = venue_bit(VENUE_VELOCITY)
    | venue_bit(VENUE_JUPITER_PERPS)
    | venue_bit(VENUE_ADRENA)
    | venue_bit(VENUE_FLASH_TRADE);

/// Is `venue_id` a real id that `flags` currently enables?
pub const fn venue_enabled_in(flags: u8, venue_id: u8) -> bool {
    let bit = venue_bit(venue_id);
    bit != 0 && (flags & bit) != 0
}

/// Reference issuance floor on net carry, in **annualised** basis points.
///
/// Derived from the runway rule in `docs/risk-spec.md`: the insurance buffer
/// must survive a sustained negative-carry regime for at least
/// `min_runway_days`, so
///
/// ```text
/// carry_floor_daily = -(buffer_ratio / min_runway_days)
///                   = -(300 bps / 30 days) = -10 bps/day
/// carry_floor_yr    = -10 * 365            = -3650 bps = -36.5 %/yr
/// ```
///
/// Against measured SOL delta-neutral carry that is a live gate, not a
/// formality: 1y -35.8 % passes by 0.7 points, 30d -43.3 % and 24h -105 % are
/// both refused. Stored as a config parameter, so `set_params` can move it
/// when the buffer ratio or the required runway changes.
pub const REFERENCE_MIN_NET_CARRY_BPS: i32 = -3_650;

// ---------------------------------------------------------------------------
// Slashable faults
//
// Enumerated so a slash is rule-based rather than discretionary
// (`docs/security.md` 2.4). The authority must name which rule was broken, and
// the reason travels in the event next to the evidence hash, so a slash that
// does not correspond to a published rule is itself visible misconduct.
// ---------------------------------------------------------------------------

/// Delta outside the band with no committed proof explaining it.
pub const SLASH_REASON_DELTA_OUT_OF_BAND: u8 = 1;
/// Carry anomaly beyond the bleed cap, unexplained by funding.
pub const SLASH_REASON_CARRY_ANOMALY: u8 = 2;
/// Venue concentration or turnover cap breached.
pub const SLASH_REASON_CAP_BREACH: u8 = 3;
/// No proof committed inside the liveness window.
pub const SLASH_REASON_LIVENESS: u8 = 4;
/// A committed proof whose hash does not reconcile against venue history.
pub const SLASH_REASON_FALSE_PROOF: u8 = 5;
pub const SLASH_REASON_MAX: u8 = SLASH_REASON_FALSE_PROOF;

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// Global protocol configuration and accounting. Singleton, PDA `["config"]`.
///
/// `authority` is expected to be a multisig or a timelock program address, not
/// a hot key. Nothing here assumes the authority is an individual signer: every
/// admin instruction is a single `Signer` check, which a Squads multisig or a
/// timelock executor satisfies transparently.
///
/// `guardian` is the separate fast-pause key from `docs/security.md` 3. It can
/// only *stop* actions -- never unpause, never move funds, never change a
/// parameter -- so it can be a smaller, faster multisig without widening the
/// blast radius.
#[account]
pub struct Config {
    pub authority: Pubkey, // 32
    /// Two-step authority handover. Zero when no handover is pending.
    pub pending_authority: Pubkey, // 32
    /// Pause-only key. Cannot unpause and cannot move value.
    pub guardian: Pubkey, // 32
    pub collateral_mint: Pubkey, // 32
    /// The synthetic dollar (pUSD). Its mint authority is this config PDA.
    pub synthetic_mint: Pubkey, // 32
    /// $POYZ. Keeper bonds are denominated in it.
    pub bond_mint: Pubkey, // 32
    /// Pyth `PriceUpdateV2` account for the collateral asset.
    pub oracle: Pubkey, // 32
    /// Token program that owns all three mints. Pinned so one token program
    /// account is enough for every instruction.
    pub token_program: Pubkey, // 32
    /// Pyth feed id the oracle account must carry.
    pub feed_id: [u8; 32], // 32
    /// Head of the rebalance proof hash chain. Zero before the first proof.
    pub last_proof_hash: [u8; 32], // 32

    /// Funding-per-staked-unit accumulator, scaled by `math::ACC_SCALE`.
    pub acc_funding_per_share: u128, // 16

    /// Collateral backing outstanding synthetic dollars.
    pub total_collateral: u64, // 8
    /// Collateral escrowed by mint requests that have not been confirmed yet.
    /// Kept out of `total_collateral` so an unconfirmed request never counts as
    /// backing.
    pub pending_collateral: u64, // 8
    /// Synthetic dollars in circulation.
    pub total_synthetic: u64, // 8
    /// Synthetic dollars escrowed by redeem requests awaiting settlement.
    pub pending_redeem_synthetic: u64, // 8
    /// Short notional last attested by a keeper.
    pub hedged_notional: u64, // 8
    /// Staked synthetic earning funding. Excludes amounts in unstake cooldown.
    pub total_staked: u64, // 8
    /// Synthetic dollars sitting in the funding vault owed to stakers.
    pub staker_funding_balance: u64, // 8
    /// Synthetic dollars in the insurance buffer.
    pub buffer_balance: u64, // 8
    /// Sum of live keeper bonds.
    pub bonded_total: u64, // 8
    /// Lifetime slashed bond, moved to the buffer bond vault.
    pub slashed_total: u64, // 8
    pub min_keeper_bond: u64,      // 8
    pub max_synthetic_supply: u64, // 8
    /// Monotonic rebalance counter; doubles as the next proof sequence.
    pub rebalance_count: u64, // 8
    pub last_proof_slot: u64,      // 8

    /// Unix time when funding first turned negative in the current regime.
    /// Zero when funding is not negative. Gates insurance buffer withdrawals.
    pub negative_funding_since: i64, // 8
    pub last_settle_at: i64, // 8
    /// When `report_venue_state` last wrote the carry and capacity numbers.
    /// Zero means never; minting is blocked until it is set, and blocked again
    /// once it is older than `max_venue_state_age_sec`.
    pub venue_state_at: i64, // 8
    /// Notional the hedge venue can currently absorb, in synthetic base units,
    /// as last reported *and clamped* to `max_reportable_capacity_notional`.
    /// Caps issuance through `max_supply_vs_capacity_bps`.
    pub venue_capacity_notional: u64, // 8
    /// Admin ceiling on what any reporter may claim the venue can absorb.
    ///
    /// This is the guardrail that makes it safe to let bonded keepers report.
    /// A reporter may understate capacity -- that only tightens issuance -- but
    /// cannot overstate it past a number the authority set. Without the clamp,
    /// an inflated capacity report opens over-issuance, and unlike a false
    /// carry report that is not something a later slash can undo: the synthetic
    /// dollars have already been minted.
    pub max_reportable_capacity_notional: u64, // 8

    pub max_price_age_sec: u32,        // 4
    pub request_ttl_sec: u32,          // 4
    pub min_settlement_delay_sec: u32, // 4
    pub unbond_cooldown_sec: u32,      // 4
    pub buffer_unlock_delay_sec: u32,  // 4
    pub unstake_cooldown_sec: u32,     // 4
    /// How long a venue-state report stays usable. Beyond it, minting stops.
    pub max_venue_state_age_sec: u32, // 4
    pub keeper_count: u32,             // 4
    /// Last reported net carry in bps, already net of venue costs and of the
    /// venue's asymmetric-funding cap. Signed: negative means the protocol pays
    /// to hold the hedge rather than being paid for it.
    pub last_net_carry_bps: i32, // 4
    /// Issuance floor. Minting is refused while `last_net_carry_bps` is below
    /// it. Signed, because a deployment may deliberately accept a small
    /// negative carry; it may not accept an unbounded one.
    pub min_net_carry_bps: i32, // 4

    pub max_conf_bps: u16,         // 2
    pub collateral_ratio_bps: u16, // 2
    pub mint_fee_bps: u16,         // 2
    pub redeem_fee_bps: u16,       // 2
    /// Outer band. Beyond this the book must be rebalanced.
    pub delta_band_bps: u16, // 2
    /// Inner target. A rebalance proof must land the book inside this, not
    /// merely back inside the outer band -- the hysteresis that stops a keeper
    /// from parking the book permanently at the edge of tolerance.
    pub delta_exit_bps: u16, // 2
    /// Emergency band. Beyond this the book is too unbalanced to issue against
    /// at all, and `mint_request` refuses regardless of everything else.
    pub delta_hard_bps: u16, // 2
    pub max_hedge_slippage_bps: u16, // 2
    /// Share of every funding settlement routed to the insurance buffer.
    pub buffer_share_bps: u16, // 2
    /// Per-call cap on insurance buffer withdrawals, as a share of the buffer.
    pub buffer_max_draw_bps: u16, // 2
    /// Ceiling on outstanding supply as a share of reported venue capacity.
    /// Issuing more synthetic than the venue can absorb means the marginal
    /// dollar is structurally unhedgeable, whatever the keeper attests.
    pub max_supply_vs_capacity_bps: u16, // 2

    pub collateral_decimals: u8, // 1
    pub synthetic_decimals: u8,  // 1
    pub bond_decimals: u8,       // 1
    /// Issuance halted. Redemption stays open on purpose: the asymmetry is the
    /// point of two flags. A stop that also blocks exits is a freeze.
    pub mint_paused: bool, // 1
    pub redeem_paused: bool,     // 1
    pub bump: u8,                // 1
    /// Bitfield of initialized token vaults; see `VAULT_FLAG_*`.
    pub vault_flags: u8, // 1
    /// Bitfield of enabled hedge venues; bit n enables venue id n.
    pub venue_flags: u8, // 1
    /// Venue named by the most recent state report. `VENUE_NONE` before any.
    pub last_venue_id: u8, // 1

    pub reserved: [u8; 25], // 25
}

impl Config {
    //  8 pubkeys                                 256
    //    feed_id + last_proof_hash                64
    //  1 u128                                     16
    // 19 u64/i64 accounting + timestamps         152
    // 10 u32/i32                                  40
    // 11 u16                                      22
    //  9 u8/bool                                   9
    //    reserved                                 25
    //                                        ---------
    pub const LEN: usize = 584;

    pub fn vaults_ready(&self) -> bool {
        self.vault_flags == VAULT_FLAGS_ALL
    }

    /// Is `venue_id` a real, currently enabled venue?
    pub fn venue_enabled(&self, venue_id: u8) -> bool {
        venue_enabled_in(self.venue_flags, venue_id)
    }
}

// ---------------------------------------------------------------------------
// Keeper
// ---------------------------------------------------------------------------

/// A Delta Keeper's registration and bond. PDA `["keeper", keeper]`.
#[account]
pub struct Keeper {
    pub keeper: Pubkey,        // 32
    pub bonded: u64,           // 8
    pub slashed: u64,          // 8
    pub proofs_committed: u64, // 8
    pub registered_at: i64,    // 8
    pub last_proof_at: i64,    // 8
    pub last_proof_slot: u64,  // 8
    pub last_bond_at: i64,     // 8
    pub active: bool,          // 1
    pub bump: u8,              // 1
    pub reserved: [u8; 14],    // 14
}

impl Keeper {
    pub const LEN: usize = 104;
}

// ---------------------------------------------------------------------------
// Mint / redeem requests
// ---------------------------------------------------------------------------

/// A two-phase mint in flight. PDA `["mint_request", user, nonce_le]`.
///
/// The account existing *is* the pending state; there is no status enum to get
/// out of sync. It is closed on confirm and on cancel, refunding rent to the
/// user either way.
#[account]
pub struct MintRequest {
    pub user: Pubkey,           // 32
    pub nonce: u64,             // 8
    pub collateral_amount: u64, // 8
    /// Notional at the request-time price. The confirm path takes the minimum
    /// of this and the confirm-time notional.
    pub quoted_notional: u64, // 8
    pub min_synthetic_out: u64, // 8
    pub quoted_price: i64,      // 8
    pub created_at: i64,        // 8
    /// After this time the keeper loses its exclusive window and the user can
    /// cancel and reclaim the collateral.
    pub deadline: i64, // 8
    pub quoted_slot: u64,       // 8
    pub quoted_expo: i32,       // 4
    pub bump: u8,               // 1
    pub reserved: [u8; 11],     // 11
}

impl MintRequest {
    pub const LEN: usize = 112;
}

/// A two-phase redeem in flight. PDA `["redeem_request", user, nonce_le]`.
#[account]
pub struct RedeemRequest {
    pub user: Pubkey,          // 32
    pub nonce: u64,            // 8
    pub synthetic_amount: u64, // 8
    /// Collateral at the request-time price. The confirm path takes the
    /// minimum of this and the confirm-time amount.
    pub quoted_collateral: u64, // 8
    pub min_collateral_out: u64, // 8
    pub quoted_price: i64,     // 8
    pub created_at: i64,       // 8
    pub deadline: i64,         // 8
    pub quoted_slot: u64,      // 8
    pub quoted_expo: i32,      // 4
    pub bump: u8,              // 1
    pub reserved: [u8; 11],    // 11
}

impl RedeemRequest {
    pub const LEN: usize = 112;
}

// ---------------------------------------------------------------------------
// Rebalance proof
// ---------------------------------------------------------------------------

/// Immutable record of one rebalance. PDA `["proof", sequence_le]`.
///
/// What the hashes commit to, and what that buys an observer, is documented on
/// `instructions::proof::commit_rebalance_proof`.
#[account]
pub struct RebalanceProof {
    pub keeper: Pubkey, // 32
    /// Keeper-supplied digest of the per-venue execution payload.
    pub venues_hash: [u8; 32], // 32
    /// Chain head before this proof.
    pub prev_hash: [u8; 32], // 32
    /// Chain head after it. Computed by the program, never supplied.
    pub this_hash: [u8; 32], // 32
    pub sequence: u64,  // 8
    pub hedged_notional: u64, // 8
    pub collateral_notional: u64, // 8
    pub oracle_publish_time: i64, // 8
    pub oracle_posted_slot: u64, // 8
    pub slot: u64,      // 8
    pub timestamp: i64, // 8
    pub oracle_price: i64, // 8
    pub oracle_conf: u64, // 8
    pub delta_bps_before: i32, // 4
    pub delta_bps_after: i32, // 4
    pub oracle_expo: i32, // 4
    /// Hedge venue identifier. See `packages/hedge-router` for the mapping.
    pub venue_id: u8, // 1
    pub bump: u8,       // 1
    pub reserved: [u8; 18], // 18
}

impl RebalanceProof {
    pub const LEN: usize = 232;
}

// ---------------------------------------------------------------------------
// Staking
// ---------------------------------------------------------------------------

/// A staker's position in the funding vault. PDA `["stake", owner]`.
///
/// Classic accumulator accounting: `entitlement = amount * acc / SCALE`, and
/// `reward_debt` is the entitlement already accounted for. Any change to
/// `amount` must first move the outstanding difference into `unclaimed`,
/// otherwise a staker could increase `amount` and retroactively earn funding
/// that accrued before they staked.
///
/// `pending_unstake` is principal that has left `amount` (and therefore stopped
/// earning) but is still inside the cooldown window.
#[account]
pub struct StakePosition {
    pub owner: Pubkey,        // 32
    pub reward_debt: u128,    // 16
    pub amount: u64,          // 8
    pub unclaimed: u64,       // 8
    pub claimed_total: u64,   // 8
    pub last_update: i64,     // 8
    pub cooldown_end: i64,    // 8
    pub pending_unstake: u64, // 8
    pub bump: u8,             // 1
    pub reserved: [u8; 7],    // 7
}

impl StakePosition {
    pub const LEN: usize = 104;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declared_lengths_match_the_structs() {
        assert_eq!(Config::LEN, 256 + 64 + 16 + 152 + 40 + 22 + 9 + 25);
        assert_eq!(Keeper::LEN, 32 + 56 + 2 + 14);
        assert_eq!(MintRequest::LEN, 32 + 64 + 4 + 1 + 11);
        assert_eq!(RedeemRequest::LEN, 32 + 64 + 4 + 1 + 11);
        assert_eq!(RebalanceProof::LEN, 128 + 72 + 12 + 2 + 18);
        assert_eq!(StakePosition::LEN, 32 + 16 + 48 + 1 + 7);
    }

    #[test]
    fn every_len_is_eight_byte_aligned() {
        for len in [
            Config::LEN,
            Keeper::LEN,
            MintRequest::LEN,
            RedeemRequest::LEN,
            RebalanceProof::LEN,
            StakePosition::LEN,
        ] {
            assert_eq!(len % 8, 0);
        }
    }

    #[test]
    fn vault_flags_cover_every_vault_group() {
        assert_eq!(VAULT_FLAGS_ALL, 0b1111);
    }

    #[test]
    fn venue_ids_match_the_cross_package_contract() {
        // _DIRECTION.md section 8-1, mirrored in idl/venues.json. Changing any
        // of these silently repoints every execution proof at the wrong venue
        // history, and nothing downstream would fail to compile.
        assert_eq!(VENUE_NONE, 0);
        assert_eq!(VENUE_VELOCITY, 1);
        assert_eq!(VENUE_JUPITER_PERPS, 2);
        assert_eq!(VENUE_ADRENA, 3);
        assert_eq!(VENUE_FLASH_TRADE, 4);
        assert_eq!(VENUE_SIMULATED, 255);
        assert_eq!(VENUE_ID_MAX, 4);
    }

    #[test]
    fn venue_zero_can_never_be_enabled_by_any_flag_value() {
        // The point of the 1-based mapping: a zeroed byte is not a venue, and
        // no flag value -- not even every bit set -- can make it one.
        assert_eq!(venue_bit(VENUE_NONE), 0);
        assert!(!venue_enabled_in(0xFF, VENUE_NONE));
        assert_eq!(VENUE_FLAGS_MASK & 0b0000_0001, 0, "bit 0 is unusable");
    }

    #[test]
    fn only_enabled_real_venues_pass() {
        let velocity_only = venue_bit(VENUE_VELOCITY);
        assert_eq!(velocity_only, 0b0000_0010, "the documented initial value");
        assert!(venue_enabled_in(velocity_only, VENUE_VELOCITY));
        // A real id that the mask does not enable is still refused.
        assert!(!venue_enabled_in(velocity_only, VENUE_JUPITER_PERPS));
        // Simulated fills are never committable, whatever the flags.
        assert!(!venue_enabled_in(0xFF, VENUE_SIMULATED));
        // Nor is any id past the assignable range.
        assert!(!venue_enabled_in(0xFF, VENUE_ID_MAX + 1));

        let both = velocity_only | venue_bit(VENUE_JUPITER_PERPS);
        assert!(venue_enabled_in(both, VENUE_JUPITER_PERPS));
        assert_eq!(VENUE_FLAGS_MASK, 0b0001_1110);
    }

    #[test]
    fn reference_carry_floor_follows_the_runway_formula() {
        // carry_floor_daily = -(buffer_ratio / min_runway_days)
        let buffer_ratio_bps: i32 = 300; // 3 % of supply held as buffer
        let min_runway_days: i32 = 30;
        let daily = -(buffer_ratio_bps / min_runway_days);
        assert_eq!(daily, -10);
        assert_eq!(daily * 365, REFERENCE_MIN_NET_CARRY_BPS);
        // The measured regimes this gate has to separate.
        assert!(-3_580 >= REFERENCE_MIN_NET_CARRY_BPS, "1y -35.8 % passes");
        assert!(
            -4_330 < REFERENCE_MIN_NET_CARRY_BPS,
            "30d -43.3 % is refused"
        );
        assert!(
            -10_500 < REFERENCE_MIN_NET_CARRY_BPS,
            "24h -105 % is refused"
        );
    }

    #[test]
    fn slash_reasons_are_contiguous_from_one() {
        assert_eq!(SLASH_REASON_DELTA_OUT_OF_BAND, 1);
        assert_eq!(SLASH_REASON_MAX, 5);
    }
}
