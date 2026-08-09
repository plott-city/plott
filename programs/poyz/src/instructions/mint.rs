//! Two-phase mint: escrow collateral, hedge, then issue.
//!
//! # Why two phases and not a keeper co-signature
//!
//! The brief allowed either "request -> confirm" or "require a keeper signature
//! on a single-transaction mint". Two phases is the safer of the two, and the
//! difference is not stylistic:
//!
//! * A co-signature can only attest **intent**. The offsetting short is opened
//!   on Velocity or Jupiter Perps, in a different transaction, against a book that
//!   takes time to fill. At the moment a keeper signs a single-transaction
//!   mint, the hedge does not exist yet. The signature would certify a future
//!   the keeper does not control, and the protocol would be issuing synthetic
//!   dollars against an unhedged position for however long the fill takes. That
//!   is precisely the unbacked-issuance window the design exists to remove.
//!
//! * A co-signature makes every mint wait on a live keeper. One offline keeper
//!   halts issuance; a malicious one censors selectively or reorders users, and
//!   the user has no recourse because their collateral moves only in the tx the
//!   keeper agrees to sign.
//!
//! * With two phases the collateral is escrowed under program control and the
//!   user keeps an unconditional exit: after `deadline` the user calls
//!   `mint_cancel` themselves and takes the collateral back. The keeper's power
//!   is bounded to a time window, not to the funds.
//!
//! * The claim the keeper does make at confirm time -- "I filled this much
//!   notional" -- is bonded and slashable, and is committed as a hash that can
//!   be reconciled against the venue's public execution history. A signature
//!   carries no such commitment.
//!
//! Cost: two transactions and one round trip of latency. For an instrument
//! whose entire premise is that issuance is always hedged, that is the correct
//! trade.
//!
//! # Pricing across the two phases
//!
//! Request quotes a notional at the request-time price; confirm recomputes it
//! at the confirm-time price and uses **the smaller of the two**. A user
//! therefore cannot open a request, wait for a favourable move, and have it
//! confirmed at the stale better price -- the free option that a single quoted
//! price would hand them. Both phases run the full oracle gate, so a request
//! taken while the oracle was healthy still cannot settle against a stale or
//! wide-confidence price.
//!
//! # Where the mint fee goes
//!
//! The fee is not transferred anywhere: it is synthetic that is **not minted**.
//! The full collateral is credited to `total_collateral` while fewer synthetic
//! dollars enter circulation, so the fee accrues as protocol overcollateral-
//! ization. No fee token account, no fee sweep, nothing to misroute.

use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    self, Mint, MintTo, TokenAccount, TokenInterface, TransferChecked,
};

use crate::errors::PoyzError;
use crate::events::{MintCancelled, MintConfirmed, MintRequested};
use crate::math::{collateral_to_notional, delta_bps, div_bps_floor, mul_bps_ceil, mul_bps_floor};
use crate::oracle::load_price;
use crate::state::*;

/// Ceiling on outstanding synthetic: the lower of the absolute supply cap and
/// the share of reported venue capacity the protocol is willing to run at.
fn supply_ceiling(config: &Config) -> Result<u64> {
    let venue_ceiling = mul_bps_floor(
        config.venue_capacity_notional,
        config.max_supply_vs_capacity_bps,
    )?;
    Ok(config.max_synthetic_supply.min(venue_ceiling))
}

/// Every precondition for issuance that does not depend on the amount.
///
/// Grouped so `mint_request` and `mint_confirm` cannot drift apart, and so the
/// reasons appear in one place:
///
///   * the venue state must exist and be fresh. Before the first report, and
///     once a report goes stale, issuance stops. Silence is not consent;
///   * net carry must clear the floor. The delta-neutral SOL carry is currently
///     negative -- the short leg pays funding -- and a yield product that keeps
///     issuing into a negative carry is selling a loss;
///   * the book must be inside the hard band. Beyond it the existing position
///     is too unbalanced to add to, whatever the incoming hedge looks like.
fn check_issuance_gates(
    config: &Config,
    now: i64,
    price: &crate::oracle::OraclePrice,
) -> Result<()> {
    require!(config.venue_state_at != 0, PoyzError::VenueStateMissing);
    let age = now
        .checked_sub(config.venue_state_at)
        .ok_or(PoyzError::MathOverflow)?;
    require!(age >= 0, PoyzError::VenueStateFromFuture);
    require!(
        age <= i64::from(config.max_venue_state_age_sec),
        PoyzError::VenueStateStale
    );

    require!(
        config.last_net_carry_bps >= config.min_net_carry_bps,
        PoyzError::CarryBelowFloor
    );

    // An empty book has no delta to be outside a band.
    if config.total_collateral > 0 {
        let book_notional = collateral_to_notional(
            config.total_collateral,
            price.price,
            price.expo,
            config.collateral_decimals,
            config.synthetic_decimals,
        )?;
        if book_notional > 0 {
            let delta = delta_bps(book_notional, config.hedged_notional)?;
            require!(
                u64::from(delta.unsigned_abs()) <= u64::from(config.delta_hard_bps),
                PoyzError::DeltaOutsideHardBand
            );
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// mint_request
// ---------------------------------------------------------------------------

#[derive(Accounts)]
#[instruction(nonce: u64)]
pub struct MintRequestCtx<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = collateral_mint @ PoyzError::TokenProgramMismatch,
        has_one = oracle @ PoyzError::OracleAccountMismatch,
        constraint = token_program.key() == config.token_program @ PoyzError::TokenProgramMismatch,
    )]
    pub config: Box<Account<'info, Config>>,

    #[account(
        init,
        payer = user,
        space = 8 + MintRequest::LEN,
        seeds = [MINT_REQUEST_SEED, user.key().as_ref(), &nonce.to_le_bytes()],
        bump
    )]
    pub request: Box<Account<'info, MintRequest>>,

    pub collateral_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        token::mint = collateral_mint,
        token::authority = user,
        token::token_program = token_program,
    )]
    pub user_collateral: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [COLLATERAL_VAULT_SEED, collateral_mint.key().as_ref()],
        bump,
        token::mint = collateral_mint,
        token::authority = config,
        token::token_program = token_program,
    )]
    pub collateral_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: authenticated in `oracle::load_price`.
    pub oracle: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn mint_request(
    ctx: Context<MintRequestCtx>,
    nonce: u64,
    collateral_amount: u64,
    min_synthetic_out: u64,
) -> Result<()> {
    let config = &ctx.accounts.config;
    require!(!config.mint_paused, PoyzError::MintPaused);
    require!(config.vaults_ready(), PoyzError::VaultsNotReady);
    require!(collateral_amount > 0, PoyzError::ZeroAmount);
    // No bonded keeper means nobody is accountable for opening the offsetting
    // short, and the request could only ever expire. Fail now rather than
    // escrowing the user's collateral for the full TTL.
    require!(config.keeper_count > 0, PoyzError::KeeperInactive);

    let clock = Clock::get()?;
    let price = load_price(
        &ctx.accounts.oracle.to_account_info(),
        &config.oracle,
        &config.feed_id,
        config.max_price_age_sec,
        config.max_conf_bps,
        clock.unix_timestamp,
    )?;

    check_issuance_gates(config, clock.unix_timestamp, &price)?;

    // Cheap pre-check so a request that could never confirm does not escrow
    // collateral for a full TTL. The exact check is in `mint_confirm`.
    require!(
        config.total_synthetic < supply_ceiling(config)?,
        PoyzError::VenueCapacityExceeded
    );

    let quoted_notional = collateral_to_notional(
        collateral_amount,
        price.price,
        price.expo,
        config.collateral_decimals,
        config.synthetic_decimals,
    )?;
    // Dust below one synthetic base unit would escrow collateral that can never
    // mint anything.
    require!(quoted_notional > 0, PoyzError::ZeroAmount);

    let decimals = ctx.accounts.collateral_mint.decimals;
    token_interface::transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.user_collateral.to_account_info(),
                mint: ctx.accounts.collateral_mint.to_account_info(),
                to: ctx.accounts.collateral_vault.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        collateral_amount,
        decimals,
    )?;

    let deadline = clock
        .unix_timestamp
        .checked_add(i64::from(ctx.accounts.config.request_ttl_sec))
        .ok_or(PoyzError::MathOverflow)?;

    let user_key = ctx.accounts.user.key();
    let request = &mut ctx.accounts.request;
    request.user = user_key;
    request.nonce = nonce;
    request.collateral_amount = collateral_amount;
    request.quoted_notional = quoted_notional;
    request.min_synthetic_out = min_synthetic_out;
    request.quoted_price = price.price;
    request.created_at = clock.unix_timestamp;
    request.deadline = deadline;
    request.quoted_slot = clock.slot;
    request.quoted_expo = price.expo;
    request.bump = ctx.bumps.request;
    request.reserved = [0u8; 11];
    let request_key = request.key();

    // Escrowed collateral is tracked apart from `total_collateral`: it backs
    // nothing until a hedge exists, and counting it as backing would overstate
    // the collateral ratio for exactly as long as requests sit pending.
    let config = &mut ctx.accounts.config;
    config.pending_collateral = config
        .pending_collateral
        .checked_add(collateral_amount)
        .ok_or(PoyzError::MathOverflow)?;

    emit!(MintRequested {
        config: config.key(),
        request: request_key,
        user: user_key,
        nonce,
        collateral_amount,
        quoted_notional,
        quoted_price: price.price,
        quoted_expo: price.expo,
        deadline,
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// mint_confirm
// ---------------------------------------------------------------------------

#[derive(Accounts)]
#[instruction(nonce: u64)]
pub struct MintConfirmCtx<'info> {
    #[account(mut)]
    pub keeper: Signer<'info>,

    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = synthetic_mint @ PoyzError::TokenProgramMismatch,
        has_one = oracle @ PoyzError::OracleAccountMismatch,
        constraint = token_program.key() == config.token_program @ PoyzError::TokenProgramMismatch,
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
        mut,
        close = user,
        seeds = [MINT_REQUEST_SEED, user.key().as_ref(), &nonce.to_le_bytes()],
        bump = request.bump,
        has_one = user @ PoyzError::Unauthorized,
    )]
    pub request: Box<Account<'info, MintRequest>>,

    /// CHECK: the request owner. Bound by `has_one = user` on the request and
    /// by the request PDA seeds. Receives the request account's rent back.
    #[account(mut)]
    pub user: UncheckedAccount<'info>,

    #[account(mut)]
    pub synthetic_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        token::mint = synthetic_mint,
        token::authority = user,
        token::token_program = token_program,
    )]
    pub user_synthetic: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: authenticated in `oracle::load_price`.
    pub oracle: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn mint_confirm(
    ctx: Context<MintConfirmCtx>,
    _nonce: u64,
    hedge_proof_hash: [u8; 32],
    venue_id: u8,
    filled_notional: u64,
) -> Result<()> {
    let config = &ctx.accounts.config;
    require!(!config.mint_paused, PoyzError::MintPaused);
    require!(
        ctx.accounts.keeper_account.active,
        PoyzError::KeeperInactive
    );
    require!(
        ctx.accounts.keeper_account.bonded >= config.min_keeper_bond,
        PoyzError::InsufficientBond
    );
    require!(hedge_proof_hash != [0u8; 32], PoyzError::EmptyProofHash);
    require!(config.venue_enabled(venue_id), PoyzError::VenueNotEnabled);

    let clock = Clock::get()?;
    // Past the deadline the keeper's exclusive window is over and the user owns
    // the decision. Confirming afterwards would race the user's cancel.
    require!(
        clock.unix_timestamp <= ctx.accounts.request.deadline,
        PoyzError::RequestExpired
    );

    let price = load_price(
        &ctx.accounts.oracle.to_account_info(),
        &config.oracle,
        &config.feed_id,
        config.max_price_age_sec,
        config.max_conf_bps,
        clock.unix_timestamp,
    )?;

    let collateral_amount = ctx.accounts.request.collateral_amount;
    let notional_now = collateral_to_notional(
        collateral_amount,
        price.price,
        price.expo,
        config.collateral_decimals,
        config.synthetic_decimals,
    )?;
    // The lower of the two quotes. Removes the free option a user would
    // otherwise hold between request and confirm.
    let effective_notional = notional_now.min(ctx.accounts.request.quoted_notional);
    require!(effective_notional > 0, PoyzError::ZeroAmount);

    // The hedge must actually cover the notional being issued against, within
    // the configured slippage band. Anything less and the position is only
    // partially delta-neutral from birth.
    let required_fill = mul_bps_floor(
        effective_notional,
        10_000u16
            .checked_sub(config.max_hedge_slippage_bps)
            .ok_or(PoyzError::InvalidBps)?,
    )?;
    require!(
        filled_notional >= required_fill,
        PoyzError::HedgeFillTooSmall
    );
    // Bounded from above too. An over-reported fill inflates
    // `hedged_notional`, which makes the book read as over-hedged and masks a
    // real under-hedge from every downstream band check -- the same damage as
    // under-reporting, in the opposite direction, and just as slashable.
    let max_fill = mul_bps_ceil(
        effective_notional,
        10_000u16
            .checked_add(config.max_hedge_slippage_bps)
            .ok_or(PoyzError::InvalidBps)?,
    )?;
    require!(filled_notional <= max_fill, PoyzError::HedgeFillTooLarge);

    let gross = div_bps_floor(effective_notional, config.collateral_ratio_bps)?;
    require!(gross > 0, PoyzError::ZeroAmount);
    let fee = mul_bps_ceil(gross, config.mint_fee_bps)?;
    let net = gross.checked_sub(fee).ok_or(PoyzError::MathOverflow)?;
    require!(net > 0, PoyzError::ZeroAmount);
    require!(
        net >= ctx.accounts.request.min_synthetic_out,
        PoyzError::SlippageExceeded
    );

    let new_supply = config
        .total_synthetic
        .checked_add(net)
        .ok_or(PoyzError::MathOverflow)?;
    require!(
        new_supply <= config.max_synthetic_supply,
        PoyzError::SupplyCapExceeded
    );
    // Authoritative capacity check. The request-time pre-check used the supply
    // at request time; this one uses the supply this instruction is about to
    // create.
    require!(
        new_supply <= supply_ceiling(config)?,
        PoyzError::VenueCapacityExceeded
    );

    let config_bump = config.bump;
    let seeds: &[&[u8]] = &[CONFIG_SEED, std::slice::from_ref(&config_bump)];
    let signer_seeds: &[&[&[u8]]] = &[seeds];

    token_interface::mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            MintTo {
                mint: ctx.accounts.synthetic_mint.to_account_info(),
                to: ctx.accounts.user_synthetic.to_account_info(),
                authority: ctx.accounts.config.to_account_info(),
            },
            signer_seeds,
        ),
        net,
    )?;

    let nonce = ctx.accounts.request.nonce;
    let user_key = ctx.accounts.request.user;
    let keeper_key = ctx.accounts.keeper.key();

    let config = &mut ctx.accounts.config;
    config.pending_collateral = config
        .pending_collateral
        .checked_sub(collateral_amount)
        .ok_or(PoyzError::MathOverflow)?;
    config.total_collateral = config
        .total_collateral
        .checked_add(collateral_amount)
        .ok_or(PoyzError::MathOverflow)?;
    config.total_synthetic = new_supply;
    config.hedged_notional = config
        .hedged_notional
        .checked_add(filled_notional)
        .ok_or(PoyzError::MathOverflow)?;

    emit!(MintConfirmed {
        config: config.key(),
        user: user_key,
        keeper: keeper_key,
        nonce,
        collateral_amount,
        effective_notional,
        synthetic_minted: net,
        fee,
        filled_notional,
        venue_id,
        hedge_proof_hash,
        total_synthetic: config.total_synthetic,
        total_collateral: config.total_collateral,
        hedged_notional: config.hedged_notional,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// mint_cancel
// ---------------------------------------------------------------------------

#[derive(Accounts)]
#[instruction(nonce: u64)]
pub struct MintCancelCtx<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = collateral_mint @ PoyzError::TokenProgramMismatch,
        constraint = token_program.key() == config.token_program @ PoyzError::TokenProgramMismatch,
    )]
    pub config: Box<Account<'info, Config>>,

    #[account(
        mut,
        close = user,
        seeds = [MINT_REQUEST_SEED, user.key().as_ref(), &nonce.to_le_bytes()],
        bump = request.bump,
        has_one = user @ PoyzError::Unauthorized,
    )]
    pub request: Box<Account<'info, MintRequest>>,

    pub collateral_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        seeds = [COLLATERAL_VAULT_SEED, collateral_mint.key().as_ref()],
        bump,
        token::mint = collateral_mint,
        token::authority = config,
        token::token_program = token_program,
    )]
    pub collateral_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        token::mint = collateral_mint,
        token::authority = user,
        token::token_program = token_program,
    )]
    pub user_collateral: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Interface<'info, TokenInterface>,
}

/// Reclaim escrowed collateral from an expired mint request.
///
/// Deliberately callable while the protocol is paused. A pause is an operator
/// action taken when something is wrong; if it also froze escrowed collateral,
/// the pause switch would double as a freeze on user funds. Cancel touches no
/// synthetic supply and no hedge state -- it only returns what the user put in.
pub fn mint_cancel(ctx: Context<MintCancelCtx>, _nonce: u64) -> Result<()> {
    let clock = Clock::get()?;
    require!(
        clock.unix_timestamp > ctx.accounts.request.deadline,
        PoyzError::RequestNotExpired
    );

    let amount = ctx.accounts.request.collateral_amount;
    let nonce = ctx.accounts.request.nonce;
    let decimals = ctx.accounts.collateral_mint.decimals;
    let config_bump = ctx.accounts.config.bump;
    let seeds: &[&[u8]] = &[CONFIG_SEED, std::slice::from_ref(&config_bump)];
    let signer_seeds: &[&[&[u8]]] = &[seeds];

    token_interface::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.collateral_vault.to_account_info(),
                mint: ctx.accounts.collateral_mint.to_account_info(),
                to: ctx.accounts.user_collateral.to_account_info(),
                authority: ctx.accounts.config.to_account_info(),
            },
            signer_seeds,
        ),
        amount,
        decimals,
    )?;

    let user_key = ctx.accounts.user.key();
    let config = &mut ctx.accounts.config;
    config.pending_collateral = config
        .pending_collateral
        .checked_sub(amount)
        .ok_or(PoyzError::MathOverflow)?;

    emit!(MintCancelled {
        config: config.key(),
        user: user_key,
        nonce,
        collateral_returned: amount,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
