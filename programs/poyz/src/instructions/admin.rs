//! Protocol configuration: initialize, parameter updates, pause, and a
//! two-step authority handover.
//!
//! Authority model. `Config::authority` is checked with a plain `Signer`
//! constraint, which is satisfied identically by a hot key, a Squads multisig
//! executing a transaction, or a timelock program's executor PDA. The program
//! deliberately does not hard-code a multisig implementation: doing so would
//! pin the protocol to one vendor's account layout forever. What the program
//! does enforce is that a compromised authority cannot do unbounded damage:
//!
//!   * fees are capped at `MAX_FEE_BPS` in code, not by policy;
//!   * the collateral ratio cannot go below 1.00x;
//!   * the delta band cannot exceed `MAX_DELTA_BAND_BPS`, and the inner exit
//!     target must sit inside it;
//!   * the oracle staleness bound cannot exceed one hour;
//!   * the insurance buffer can only ever move to the funding vault, never to
//!     an arbitrary destination (see `instructions::buffer`);
//!   * authority handover is two-step, so a typo'd address does not brick the
//!     protocol.
//!
//! Upgrade authority for the program itself is a deployment-time decision and
//! is documented in the package README, not encoded here.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::program_option::COption;
use anchor_spl::token_interface::{Mint, TokenInterface};

use crate::errors::PoyzError;
use crate::events::{
    AuthorityTransferProposed, AuthorityTransferred, GuardianChanged, OracleUpdated, ParamsUpdated,
    PauseChanged, ProtocolInitialized, VenueStateReported,
};
use crate::oracle::validate_oracle_account;
use crate::state::*;

/// Everything `initialize` needs beyond the accounts. Grouped into one struct
/// so the instruction keeps a readable signature and the IDL exposes named
/// fields rather than a positional argument list.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct InitializeParams {
    pub feed_id: [u8; 32],
    /// Pause-only key (`docs/security.md` 3). May equal the authority if a
    /// deployment chooses not to run a separate guardian.
    pub guardian: Pubkey,
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
    pub max_supply_vs_capacity_bps: u16,
    pub min_keeper_bond: u64,
    pub max_synthetic_supply: u64,
    pub request_ttl_sec: u32,
    pub min_settlement_delay_sec: u32,
    pub unbond_cooldown_sec: u32,
    pub buffer_unlock_delay_sec: u32,
    pub unstake_cooldown_sec: u32,
    pub max_venue_state_age_sec: u32,
    /// Issuance floor on net carry, in annualised bps. Signed.
    pub min_net_carry_bps: i32,
    /// Admin ceiling on any reported venue capacity.
    pub max_reportable_capacity_notional: u64,
    /// Bitmask of enabled hedge venues; bit n enables venue id n.
    pub venue_flags: u8,
}

/// Partial parameter update. Every field is optional; `None` leaves the current
/// value untouched. The whole resulting config is re-validated afterwards, so
/// there is no ordering in which a sequence of partial updates reaches an
/// invalid state.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct UpdateParams {
    pub max_price_age_sec: Option<u32>,
    pub max_conf_bps: Option<u16>,
    pub collateral_ratio_bps: Option<u16>,
    pub mint_fee_bps: Option<u16>,
    pub redeem_fee_bps: Option<u16>,
    pub delta_band_bps: Option<u16>,
    pub delta_exit_bps: Option<u16>,
    pub delta_hard_bps: Option<u16>,
    pub max_hedge_slippage_bps: Option<u16>,
    pub buffer_share_bps: Option<u16>,
    pub buffer_max_draw_bps: Option<u16>,
    pub max_supply_vs_capacity_bps: Option<u16>,
    pub min_keeper_bond: Option<u64>,
    pub max_synthetic_supply: Option<u64>,
    pub request_ttl_sec: Option<u32>,
    pub min_settlement_delay_sec: Option<u32>,
    pub unbond_cooldown_sec: Option<u32>,
    pub buffer_unlock_delay_sec: Option<u32>,
    pub unstake_cooldown_sec: Option<u32>,
    pub max_venue_state_age_sec: Option<u32>,
    pub min_net_carry_bps: Option<i32>,
    pub max_reportable_capacity_notional: Option<u64>,
    pub venue_flags: Option<u8>,
}

/// Bounds that hold for any reachable config, checked after construction and
/// after every update.
fn validate_config(config: &Config) -> Result<()> {
    require!(
        config.max_price_age_sec > 0 && config.max_price_age_sec <= MAX_PRICE_AGE_SEC_LIMIT,
        PoyzError::InvalidPriceAge
    );
    require!(
        config.collateral_ratio_bps >= MIN_COLLATERAL_RATIO_BPS
            && config.collateral_ratio_bps <= MAX_COLLATERAL_RATIO_BPS,
        PoyzError::InvalidCollateralRatio
    );
    require!(
        config.delta_band_bps > 0 && config.delta_band_bps <= MAX_DELTA_BAND_BPS,
        PoyzError::InvalidDeltaThreshold
    );
    // Three bands, strictly ordered: exit <= trigger <= hard.
    // The inner target must sit inside the outer band, otherwise the
    // hysteresis collapses and a keeper may park the book at the edge of
    // tolerance forever. The hard band must sit outside the trigger, otherwise
    // the emergency stop fires before the ordinary rebalance signal does.
    require!(
        config.delta_exit_bps > 0 && config.delta_exit_bps <= config.delta_band_bps,
        PoyzError::InvalidDeltaThreshold
    );
    require!(
        config.delta_hard_bps >= config.delta_band_bps
            && config.delta_hard_bps <= MAX_DELTA_BAND_BPS,
        PoyzError::InvalidDeltaThreshold
    );
    require!(config.mint_fee_bps <= MAX_FEE_BPS, PoyzError::InvalidBps);
    require!(config.redeem_fee_bps <= MAX_FEE_BPS, PoyzError::InvalidBps);
    // A zero confidence bound would reject every real Pyth update; a bound at
    // or above 100 % would accept any of them. Both are misconfigurations.
    require!(
        config.max_conf_bps > 0 && config.max_conf_bps < 10_000,
        PoyzError::InvalidBps
    );
    require!(
        config.max_hedge_slippage_bps < 10_000,
        PoyzError::InvalidBps
    );
    require!(config.buffer_share_bps <= 10_000, PoyzError::InvalidBps);
    require!(
        config.buffer_max_draw_bps > 0 && config.buffer_max_draw_bps <= 10_000,
        PoyzError::InvalidBps
    );
    // Issuing above the venue's hedgeable capacity means the marginal dollar is
    // structurally unhedgeable, so the share is capped at 100 % and cannot be
    // switched off by setting it to zero.
    require!(
        config.max_supply_vs_capacity_bps > 0 && config.max_supply_vs_capacity_bps <= 10_000,
        PoyzError::InvalidBps
    );
    require!(config.max_venue_state_age_sec > 0, PoyzError::ZeroAmount);
    // At least one real venue must be enabled, and no bit may be set outside
    // the assignable id range. Bit 0 is included in the rejection: venue id 0
    // is the "unset" sentinel, so a flag word that enables it is a mistake
    // rather than a configuration.
    require!(
        config.venue_flags != 0 && config.venue_flags & !VENUE_FLAGS_MASK == 0,
        PoyzError::VenueNotEnabled
    );
    require!(config.min_keeper_bond > 0, PoyzError::ZeroAmount);
    require!(config.max_synthetic_supply > 0, PoyzError::ZeroAmount);
    require!(config.request_ttl_sec > 0, PoyzError::ZeroAmount);
    Ok(())
}

// ---------------------------------------------------------------------------
// initialize
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = 8 + Config::LEN,
        seeds = [CONFIG_SEED],
        bump
    )]
    pub config: Box<Account<'info, Config>>,

    pub collateral_mint: Box<InterfaceAccount<'info, Mint>>,

    /// The synthetic dollar. Its mint authority must already be the config PDA
    /// (a derivable address), and it must have no freeze authority.
    pub synthetic_mint: Box<InterfaceAccount<'info, Mint>>,

    /// $POYZ. Keeper bonds are denominated in it.
    pub bond_mint: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: validated by `oracle::validate_oracle_account` -- owner must be
    /// the Pyth receiver program, discriminator must be PriceUpdateV2, and the
    /// feed id must match `params.feed_id`.
    pub oracle: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn initialize(ctx: Context<Initialize>, params: InitializeParams) -> Result<()> {
    let config_key = ctx.accounts.config.key();
    let token_program_key = ctx.accounts.token_program.key();

    // One token program for all three mints. Mixing SPL Token and Token-2022
    // would force a second token program account into every instruction for no
    // product benefit, and silently changes which CPI helper is correct.
    for mint in [
        &ctx.accounts.collateral_mint,
        &ctx.accounts.synthetic_mint,
        &ctx.accounts.bond_mint,
    ] {
        require_keys_eq!(
            *mint.to_account_info().owner,
            token_program_key,
            PoyzError::TokenProgramMismatch
        );
    }

    require!(
        ctx.accounts.collateral_mint.decimals <= MAX_MINT_DECIMALS
            && ctx.accounts.synthetic_mint.decimals <= MAX_MINT_DECIMALS
            && ctx.accounts.bond_mint.decimals <= MAX_MINT_DECIMALS,
        PoyzError::InvalidDecimals
    );

    // Without this the protocol cannot issue, and worse, somebody else could.
    require!(
        ctx.accounts.synthetic_mint.mint_authority == COption::Some(config_key),
        PoyzError::InvalidMintAuthority
    );
    // A freeze authority on the synthetic dollar is a censorship switch over
    // every holder. The protocol refuses to launch against one.
    require!(
        ctx.accounts.synthetic_mint.freeze_authority.is_none(),
        PoyzError::FreezeAuthoritySet
    );

    validate_oracle_account(&ctx.accounts.oracle.to_account_info(), &params.feed_id)?;

    let config = &mut ctx.accounts.config;
    config.authority = ctx.accounts.authority.key();
    config.pending_authority = Pubkey::default();
    require_keys_neq!(params.guardian, Pubkey::default(), PoyzError::Unauthorized);
    config.guardian = params.guardian;
    config.collateral_mint = ctx.accounts.collateral_mint.key();
    config.synthetic_mint = ctx.accounts.synthetic_mint.key();
    config.bond_mint = ctx.accounts.bond_mint.key();
    config.oracle = ctx.accounts.oracle.key();
    config.token_program = token_program_key;
    config.feed_id = params.feed_id;
    config.last_proof_hash = [0u8; 32];

    config.acc_funding_per_share = 0;
    config.total_collateral = 0;
    config.pending_collateral = 0;
    config.total_synthetic = 0;
    config.pending_redeem_synthetic = 0;
    config.hedged_notional = 0;
    config.total_staked = 0;
    config.staker_funding_balance = 0;
    config.buffer_balance = 0;
    config.bonded_total = 0;
    config.slashed_total = 0;
    config.min_keeper_bond = params.min_keeper_bond;
    config.max_synthetic_supply = params.max_synthetic_supply;
    config.rebalance_count = 0;
    config.last_proof_slot = 0;
    config.negative_funding_since = 0;
    config.last_settle_at = 0;

    config.max_price_age_sec = params.max_price_age_sec;
    config.request_ttl_sec = params.request_ttl_sec;
    config.min_settlement_delay_sec = params.min_settlement_delay_sec;
    config.unbond_cooldown_sec = params.unbond_cooldown_sec;
    config.buffer_unlock_delay_sec = params.buffer_unlock_delay_sec;
    config.unstake_cooldown_sec = params.unstake_cooldown_sec;
    config.keeper_count = 0;

    config.venue_state_at = 0;
    config.venue_capacity_notional = 0;
    config.last_venue_id = VENUE_NONE;
    config.last_net_carry_bps = 0;
    config.min_net_carry_bps = params.min_net_carry_bps;
    config.max_reportable_capacity_notional = params.max_reportable_capacity_notional;
    config.max_venue_state_age_sec = params.max_venue_state_age_sec;
    config.venue_flags = params.venue_flags;
    config.max_supply_vs_capacity_bps = params.max_supply_vs_capacity_bps;

    config.max_conf_bps = params.max_conf_bps;
    config.collateral_ratio_bps = params.collateral_ratio_bps;
    config.mint_fee_bps = params.mint_fee_bps;
    config.redeem_fee_bps = params.redeem_fee_bps;
    config.delta_band_bps = params.delta_band_bps;
    config.delta_exit_bps = params.delta_exit_bps;
    config.delta_hard_bps = params.delta_hard_bps;
    config.max_hedge_slippage_bps = params.max_hedge_slippage_bps;
    config.buffer_share_bps = params.buffer_share_bps;
    config.buffer_max_draw_bps = params.buffer_max_draw_bps;

    config.collateral_decimals = ctx.accounts.collateral_mint.decimals;
    config.synthetic_decimals = ctx.accounts.synthetic_mint.decimals;
    config.bond_decimals = ctx.accounts.bond_mint.decimals;
    // Launches fully paused. The vault token accounts do not exist yet, and
    // an operator has to unpause deliberately once they do.
    config.mint_paused = true;
    config.redeem_paused = true;
    config.bump = ctx.bumps.config;
    config.vault_flags = 0;
    config.reserved = [0u8; 25];

    validate_config(config)?;

    emit!(ProtocolInitialized {
        config: config_key,
        authority: config.authority,
        collateral_mint: config.collateral_mint,
        synthetic_mint: config.synthetic_mint,
        bond_mint: config.bond_mint,
        oracle: config.oracle,
        feed_id: config.feed_id,
        collateral_ratio_bps: config.collateral_ratio_bps,
        delta_band_bps: config.delta_band_bps,
        min_keeper_bond: config.min_keeper_bond,
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// set_params
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct AdminOnly<'info> {
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = authority @ PoyzError::Unauthorized,
    )]
    pub config: Box<Account<'info, Config>>,
}

pub fn set_params(ctx: Context<AdminOnly>, params: UpdateParams) -> Result<()> {
    let config = &mut ctx.accounts.config;

    if let Some(v) = params.max_price_age_sec {
        config.max_price_age_sec = v;
    }
    if let Some(v) = params.max_conf_bps {
        config.max_conf_bps = v;
    }
    if let Some(v) = params.collateral_ratio_bps {
        config.collateral_ratio_bps = v;
    }
    if let Some(v) = params.mint_fee_bps {
        config.mint_fee_bps = v;
    }
    if let Some(v) = params.redeem_fee_bps {
        config.redeem_fee_bps = v;
    }
    if let Some(v) = params.delta_band_bps {
        config.delta_band_bps = v;
    }
    if let Some(v) = params.delta_exit_bps {
        config.delta_exit_bps = v;
    }
    if let Some(v) = params.delta_hard_bps {
        config.delta_hard_bps = v;
    }
    if let Some(v) = params.max_supply_vs_capacity_bps {
        config.max_supply_vs_capacity_bps = v;
    }
    if let Some(v) = params.max_venue_state_age_sec {
        config.max_venue_state_age_sec = v;
    }
    if let Some(v) = params.min_net_carry_bps {
        config.min_net_carry_bps = v;
    }
    if let Some(v) = params.max_reportable_capacity_notional {
        config.max_reportable_capacity_notional = v;
    }
    if let Some(v) = params.venue_flags {
        config.venue_flags = v;
    }
    if let Some(v) = params.max_hedge_slippage_bps {
        config.max_hedge_slippage_bps = v;
    }
    if let Some(v) = params.buffer_share_bps {
        config.buffer_share_bps = v;
    }
    if let Some(v) = params.buffer_max_draw_bps {
        config.buffer_max_draw_bps = v;
    }
    if let Some(v) = params.min_keeper_bond {
        config.min_keeper_bond = v;
    }
    if let Some(v) = params.max_synthetic_supply {
        config.max_synthetic_supply = v;
    }
    if let Some(v) = params.request_ttl_sec {
        config.request_ttl_sec = v;
    }
    if let Some(v) = params.min_settlement_delay_sec {
        config.min_settlement_delay_sec = v;
    }
    if let Some(v) = params.unbond_cooldown_sec {
        config.unbond_cooldown_sec = v;
    }
    if let Some(v) = params.buffer_unlock_delay_sec {
        config.buffer_unlock_delay_sec = v;
    }
    if let Some(v) = params.unstake_cooldown_sec {
        config.unstake_cooldown_sec = v;
    }

    validate_config(config)?;

    emit!(ParamsUpdated {
        config: config.key(),
        authority: ctx.accounts.authority.key(),
        max_price_age_sec: config.max_price_age_sec,
        max_conf_bps: config.max_conf_bps,
        collateral_ratio_bps: config.collateral_ratio_bps,
        mint_fee_bps: config.mint_fee_bps,
        redeem_fee_bps: config.redeem_fee_bps,
        delta_band_bps: config.delta_band_bps,
        delta_exit_bps: config.delta_exit_bps,
        delta_hard_bps: config.delta_hard_bps,
        max_hedge_slippage_bps: config.max_hedge_slippage_bps,
        buffer_share_bps: config.buffer_share_bps,
        buffer_max_draw_bps: config.buffer_max_draw_bps,
        min_keeper_bond: config.min_keeper_bond,
        max_synthetic_supply: config.max_synthetic_supply,
        request_ttl_sec: config.request_ttl_sec,
        min_settlement_delay_sec: config.min_settlement_delay_sec,
        unbond_cooldown_sec: config.unbond_cooldown_sec,
        buffer_unlock_delay_sec: config.buffer_unlock_delay_sec,
        unstake_cooldown_sec: config.unstake_cooldown_sec,
        max_supply_vs_capacity_bps: config.max_supply_vs_capacity_bps,
        max_venue_state_age_sec: config.max_venue_state_age_sec,
        min_net_carry_bps: config.min_net_carry_bps,
        max_reportable_capacity_notional: config.max_reportable_capacity_notional,
        venue_flags: config.venue_flags,
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// set_paused
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct SetPaused<'info> {
    /// Either the authority or the guardian. Checked in the handler, because
    /// which one signed decides what the instruction is allowed to do.
    pub signer: Signer<'info>,

    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
    )]
    pub config: Box<Account<'info, Config>>,
}

/// Set the mint and redeem circuit breakers independently.
///
/// Two flags rather than one, because the useful crisis response is asymmetric:
/// stop issuing, keep letting people out. A single flag forces an operator to
/// choose between leaving issuance open during an incident and freezing
/// redemptions, and freezing redemptions is how a synthetic dollar turns a
/// scare into a run.
///
/// The guardian may pause but never unpause. A fast key that can only stop
/// actions is a small blast radius; the same key able to re-open the protocol
/// would be a second authority.
pub fn set_paused(ctx: Context<SetPaused>, mint_paused: bool, redeem_paused: bool) -> Result<()> {
    let signer = ctx.accounts.signer.key();
    let config = &mut ctx.accounts.config;

    let is_authority = signer == config.authority;
    let is_guardian = signer == config.guardian;
    require!(is_authority || is_guardian, PoyzError::Unauthorized);

    // Guardian transitions must be monotonic toward "more paused".
    if !is_authority {
        require!(
            (mint_paused || !config.mint_paused) && (redeem_paused || !config.redeem_paused),
            PoyzError::GuardianCannotUnpause
        );
    }

    // Unpausing before the vault token accounts exist would expose mint and
    // redeem paths whose accounts cannot be resolved.
    if !mint_paused || !redeem_paused {
        require!(config.vaults_ready(), PoyzError::VaultsNotReady);
    }

    config.mint_paused = mint_paused;
    config.redeem_paused = redeem_paused;

    emit!(PauseChanged {
        config: config.key(),
        signer,
        mint_paused,
        redeem_paused,
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// set_guardian
// ---------------------------------------------------------------------------

pub fn set_guardian(ctx: Context<AdminOnly>, guardian: Pubkey) -> Result<()> {
    require_keys_neq!(guardian, Pubkey::default(), PoyzError::Unauthorized);

    let authority_key = ctx.accounts.authority.key();
    let config = &mut ctx.accounts.config;
    let previous = config.guardian;
    config.guardian = guardian;

    emit!(GuardianChanged {
        config: config.key(),
        authority: authority_key,
        previous_guardian: previous,
        guardian,
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// report_venue_state
// ---------------------------------------------------------------------------

/// Sanity bound on a reported carry: +/- 10000 % annualised. Beyond that the
/// reporter has a units bug, not a market observation.
const CARRY_BPS_LIMIT: i32 = 1_000_000;

#[derive(Accounts)]
pub struct ReportVenueState<'info> {
    /// The authority, or an active bonded keeper. Which one decides nothing
    /// about what may be written -- both are clamped identically -- only
    /// whether the call is allowed at all.
    pub signer: Signer<'info>,

    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
    )]
    pub config: Box<Account<'info, Config>>,

    /// Required when the signer is a keeper, omitted when it is the authority.
    #[account(
        seeds = [KEEPER_SEED, signer.key().as_ref()],
        bump,
    )]
    pub keeper_account: Option<Box<Account<'info, Keeper>>>,
}

/// Publish the hedge venue's current net carry and hedgeable capacity.
///
/// This is the input to two on-chain gates that together implement "do not
/// issue when the trade does not work":
///
///   * `min_net_carry_bps` -- minting is refused while carry sits below the
///     floor. As of the current SOL market that is not hypothetical: the
///     delta-neutral carry is negative, meaning the short leg *pays* funding.
///     A protocol that keeps issuing into that is selling a yield product that
///     loses money by construction.
///   * `max_supply_vs_capacity_bps` -- minting is refused once outstanding
///     supply reaches the reported share of what the venue can absorb. With a
///     thin venue book, the marginal issued dollar is unhedgeable no matter
///     what any keeper attests.
///
/// Both gates are only as good as this report, which is why it carries a
/// timestamp and both gates fail closed once it is older than
/// `max_venue_state_age_sec`, or before it has ever been written. Silence stops
/// issuance; it does not permit it.
///
/// # Who may report, and why it is not admin-only
///
/// The authority, or any **active bonded keeper**. Keeper access is the point:
/// the natural caller is the `delta-keeper` daemon, and the alternative --
/// handing that daemon the admin key -- would also hand it `set_params`,
/// `set_paused`, `set_oracle` and `transfer_authority`. That contradicts the
/// trust model in `docs/architecture.md` 4 (program owns, keeper is a
/// low-trust delegate that cannot withdraw) and makes the permissionless
/// `poyz keeper run` product impossible, because running a keeper would mean
/// becoming an admin.
///
/// The remaining option -- an always-online admin process -- trades a bounded
/// risk for an unbounded one: a permanently hot admin key is the single largest
/// attack surface in the system, and the venue this protocol hedges on lost its
/// admin authority to a durable-nonce attack four months ago.
///
/// A keeper reporting here is making the same *kind* of claim it already makes
/// in `commit_rebalance_proof`, under the same bond, and a false carry report
/// is `SLASH_REASON_CARRY_ANOMALY`.
///
/// # The capacity clamp
///
/// Carry and capacity are not symmetric risks. A false carry report is caught
/// after the fact and punished by slashing. A false *capacity* report opens
/// over-issuance, and slashing cannot undo that -- the synthetic dollars exist.
/// So `capacity_notional` is clamped to `max_reportable_capacity_notional`,
/// which only the authority sets. A reporter may understate capacity, which
/// only tightens issuance and is therefore not an attack; it cannot overstate
/// it past the authority's ceiling.
///
/// # Units
///
/// `net_carry_bps` is **annualised** basis points, signed, and must already be
/// net of the venue's asymmetric-funding cap (Velocity pays at most one third
/// of held equity per period). Reporting the headline funding rate instead
/// over-states the carry and is a `SLASH_REASON_CARRY_ANOMALY` fault.
///
/// The unit is pinned here because the gate is meaningless without it: the
/// floor it is compared against, `min_net_carry_bps`, is derived from the
/// buffer runway rule and defaults to `state::REFERENCE_MIN_NET_CARRY_BPS`
/// (-3650 bps/yr = -(3 % buffer / 30 days runway) annualised).
pub fn report_venue_state(
    ctx: Context<ReportVenueState>,
    venue_id: u8,
    net_carry_bps: i32,
    capacity_notional: u64,
) -> Result<()> {
    let signer = ctx.accounts.signer.key();
    let config = &ctx.accounts.config;

    let is_authority = signer == config.authority;
    if !is_authority {
        let keeper = ctx
            .accounts
            .keeper_account
            .as_ref()
            .ok_or(PoyzError::NotAuthorizedReporter)?;
        // The PDA seeds already bind the account to this signer; these check
        // that the keeper is one the protocol currently stands behind.
        require_keys_eq!(keeper.keeper, signer, PoyzError::NotAuthorizedReporter);
        require!(keeper.active, PoyzError::KeeperInactive);
        require!(
            keeper.bonded >= config.min_keeper_bond,
            PoyzError::InsufficientBond
        );
    }

    require!(config.venue_enabled(venue_id), PoyzError::VenueNotEnabled);
    require!(
        net_carry_bps.abs() <= CARRY_BPS_LIMIT,
        PoyzError::CarryOutOfRange
    );

    let clock = Clock::get()?;
    let now = clock.unix_timestamp;
    // Several keepers may report concurrently. Monotonicity stops a stale
    // report from a lagging reporter overwriting a fresher one and silently
    // re-opening a gate that had already closed.
    require!(
        now >= config.venue_state_at,
        PoyzError::VenueStateNotMonotonic
    );

    let effective_capacity = capacity_notional.min(config.max_reportable_capacity_notional);

    let config = &mut ctx.accounts.config;
    config.last_venue_id = venue_id;
    config.last_net_carry_bps = net_carry_bps;
    config.venue_capacity_notional = effective_capacity;
    config.venue_state_at = now;

    // The negative-funding regime clock lives here rather than in
    // `settle_funding`, so exactly one instruction writes it. A single positive
    // report clears it: the insurance-buffer playbook is for a sustained
    // regime, not for one bad window.
    if net_carry_bps < 0 {
        if config.negative_funding_since == 0 {
            config.negative_funding_since = now;
        }
    } else {
        config.negative_funding_since = 0;
    }

    emit!(VenueStateReported {
        config: config.key(),
        reporter: signer,
        reporter_is_authority: is_authority,
        venue_id,
        net_carry_bps,
        reported_capacity: capacity_notional,
        capacity_notional: effective_capacity,
        negative_funding_since: config.negative_funding_since,
        slot: clock.slot,
        timestamp: now,
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// set_oracle
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct SetOracle<'info> {
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = authority @ PoyzError::Unauthorized,
    )]
    pub config: Box<Account<'info, Config>>,

    /// CHECK: validated by `oracle::validate_oracle_account` before it is
    /// stored -- owner, discriminator and feed id must all check out.
    pub oracle: UncheckedAccount<'info>,
}

/// Repoint the protocol at a different Pyth price update account.
///
/// Needed because a price feed account is not a permanent address: Pyth shards
/// feeds, and a receiver redeployment produces new accounts. Without this the
/// protocol would need a program upgrade to follow its own oracle, which is a
/// far heavier and riskier operation than a validated pointer change.
///
/// The new account is authenticated the same way `initialize` authenticates the
/// first one, so the authority cannot point the protocol at an account that is
/// not a genuine Pyth update for the declared feed.
pub fn set_oracle(ctx: Context<SetOracle>, feed_id: [u8; 32]) -> Result<()> {
    validate_oracle_account(&ctx.accounts.oracle.to_account_info(), &feed_id)?;

    let oracle_key = ctx.accounts.oracle.key();
    let config = &mut ctx.accounts.config;
    let previous = config.oracle;
    config.oracle = oracle_key;
    config.feed_id = feed_id;

    emit!(OracleUpdated {
        config: config.key(),
        authority: ctx.accounts.authority.key(),
        previous_oracle: previous,
        oracle: oracle_key,
        feed_id,
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// authority handover (two-step)
// ---------------------------------------------------------------------------

pub fn transfer_authority(ctx: Context<AdminOnly>, new_authority: Pubkey) -> Result<()> {
    let config = &mut ctx.accounts.config;
    config.pending_authority = new_authority;

    emit!(AuthorityTransferProposed {
        config: config.key(),
        current_authority: config.authority,
        pending_authority: new_authority,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct AcceptAuthority<'info> {
    pub pending_authority: Signer<'info>,

    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
    )]
    pub config: Box<Account<'info, Config>>,
}

/// The proposed authority proves control of the key before it becomes the
/// authority. A one-step transfer to a mistyped or unusable address would leave
/// the protocol with no way to set parameters, pause, or slash.
pub fn accept_authority(ctx: Context<AcceptAuthority>) -> Result<()> {
    let config = &mut ctx.accounts.config;
    require!(
        config.pending_authority != Pubkey::default(),
        PoyzError::NoPendingAuthority
    );
    require_keys_eq!(
        ctx.accounts.pending_authority.key(),
        config.pending_authority,
        PoyzError::NotPendingAuthority
    );

    let previous = config.authority;
    config.authority = config.pending_authority;
    config.pending_authority = Pubkey::default();

    emit!(AuthorityTransferred {
        config: config.key(),
        previous_authority: previous,
        new_authority: config.authority,
    });

    Ok(())
}
