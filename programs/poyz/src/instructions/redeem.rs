//! Two-phase redeem: escrow synthetic, unwind the short, then release
//! collateral. The exact mirror of the mint path, for the same reason.
//!
//! Releasing collateral before the matching share of the perp short is closed
//! would leave the remaining holders over-hedged: the book would still be short
//! notional it no longer has spot against, and every subsequent price move
//! would show up as a loss to everyone who did not redeem. So the collateral
//! only moves once a bonded keeper attests the unwind, and the user's exit
//! guarantee is the same as on mint -- after `deadline`, `redeem_cancel`
//! returns the escrowed synthetic unconditionally, pause or no pause.
//!
//! # Cooldown
//!
//! `min_settlement_delay_sec` gates confirm, not cancel. Two purposes:
//!
//!   * an unwind takes real time on the venue, and a confirm in the same slot
//!     as the request could not have been preceded by one;
//!   * it removes the same-block sandwich: request at price P, push the oracle
//!     inside one block, confirm at the moved price. With a delay the attacker
//!     must hold the manipulation across the window, against the confidence
//!     and staleness gates, which is a materially harder and costlier attack.
//!
//! # Pricing and fees
//!
//! Collateral released is the **smaller** of the request-time and confirm-time
//! valuations, so waiting for a favourable move is not a free option. The
//! redeem fee is charged by burning the full `synthetic_amount` while valuing
//! only `synthetic_amount - fee` in collateral: the difference stays in the
//! vault as protocol overcollateralization, exactly like the mint fee.

use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    self, Burn, Mint, TokenAccount, TokenInterface, TransferChecked,
};

use crate::errors::PoyzError;
use crate::events::{RedeemCancelled, RedeemConfirmed, RedeemRequested};
use crate::math::{mul_bps_ceil, mul_bps_floor, notional_to_collateral};
use crate::oracle::load_price;
use crate::state::*;

// ---------------------------------------------------------------------------
// redeem_request
// ---------------------------------------------------------------------------

#[derive(Accounts)]
#[instruction(nonce: u64)]
pub struct RedeemRequestCtx<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

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
        init,
        payer = user,
        space = 8 + RedeemRequest::LEN,
        seeds = [REDEEM_REQUEST_SEED, user.key().as_ref(), &nonce.to_le_bytes()],
        bump
    )]
    pub request: Box<Account<'info, RedeemRequest>>,

    pub synthetic_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        token::mint = synthetic_mint,
        token::authority = user,
        token::token_program = token_program,
    )]
    pub user_synthetic: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [REDEEM_ESCROW_SEED],
        bump,
        token::mint = synthetic_mint,
        token::authority = config,
        token::token_program = token_program,
    )]
    pub redeem_escrow: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: authenticated in `oracle::load_price`.
    pub oracle: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn redeem_request(
    ctx: Context<RedeemRequestCtx>,
    nonce: u64,
    synthetic_amount: u64,
    min_collateral_out: u64,
) -> Result<()> {
    let config = &ctx.accounts.config;
    require!(!config.redeem_paused, PoyzError::RedeemPaused);
    require!(config.vaults_ready(), PoyzError::VaultsNotReady);
    require!(synthetic_amount > 0, PoyzError::ZeroAmount);
    require!(config.keeper_count > 0, PoyzError::KeeperInactive);

    // Every in-flight redeem counts against outstanding supply, otherwise a
    // burst of requests could collectively claim more collateral than exists.
    let in_flight = config
        .pending_redeem_synthetic
        .checked_add(synthetic_amount)
        .ok_or(PoyzError::MathOverflow)?;
    require!(
        in_flight <= config.total_synthetic,
        PoyzError::RedeemExceedsSupply
    );

    let clock = Clock::get()?;
    let price = load_price(
        &ctx.accounts.oracle.to_account_info(),
        &config.oracle,
        &config.feed_id,
        config.max_price_age_sec,
        config.max_conf_bps,
        clock.unix_timestamp,
    )?;

    let fee = mul_bps_ceil(synthetic_amount, config.redeem_fee_bps)?;
    let net_synthetic = synthetic_amount
        .checked_sub(fee)
        .ok_or(PoyzError::MathOverflow)?;
    require!(net_synthetic > 0, PoyzError::ZeroAmount);

    let quoted_collateral = notional_to_collateral(
        net_synthetic,
        price.price,
        price.expo,
        config.collateral_decimals,
        config.synthetic_decimals,
    )?;
    require!(quoted_collateral > 0, PoyzError::ZeroAmount);

    let decimals = ctx.accounts.synthetic_mint.decimals;
    token_interface::transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.user_synthetic.to_account_info(),
                mint: ctx.accounts.synthetic_mint.to_account_info(),
                to: ctx.accounts.redeem_escrow.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        synthetic_amount,
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
    request.synthetic_amount = synthetic_amount;
    request.quoted_collateral = quoted_collateral;
    request.min_collateral_out = min_collateral_out;
    request.quoted_price = price.price;
    request.created_at = clock.unix_timestamp;
    request.deadline = deadline;
    request.quoted_slot = clock.slot;
    request.quoted_expo = price.expo;
    request.bump = ctx.bumps.request;
    request.reserved = [0u8; 11];
    let request_key = request.key();

    let config = &mut ctx.accounts.config;
    config.pending_redeem_synthetic = in_flight;

    emit!(RedeemRequested {
        config: config.key(),
        request: request_key,
        user: user_key,
        nonce,
        synthetic_amount,
        quoted_collateral,
        quoted_price: price.price,
        quoted_expo: price.expo,
        deadline,
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// redeem_confirm
// ---------------------------------------------------------------------------

#[derive(Accounts)]
#[instruction(nonce: u64)]
pub struct RedeemConfirmCtx<'info> {
    #[account(mut)]
    pub keeper: Signer<'info>,

    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = collateral_mint @ PoyzError::TokenProgramMismatch,
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
        seeds = [REDEEM_REQUEST_SEED, user.key().as_ref(), &nonce.to_le_bytes()],
        bump = request.bump,
        has_one = user @ PoyzError::Unauthorized,
    )]
    pub request: Box<Account<'info, RedeemRequest>>,

    /// CHECK: the request owner, bound by `has_one = user` and by the request
    /// PDA seeds. Receives the collateral and the request account's rent.
    #[account(mut)]
    pub user: UncheckedAccount<'info>,

    #[account(mut)]
    pub synthetic_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        seeds = [REDEEM_ESCROW_SEED],
        bump,
        token::mint = synthetic_mint,
        token::authority = config,
        token::token_program = token_program,
    )]
    pub redeem_escrow: Box<InterfaceAccount<'info, TokenAccount>>,

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

    /// CHECK: authenticated in `oracle::load_price`.
    pub oracle: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn redeem_confirm(
    ctx: Context<RedeemConfirmCtx>,
    _nonce: u64,
    unwind_proof_hash: [u8; 32],
    venue_id: u8,
    unwound_notional: u64,
) -> Result<()> {
    let config = &ctx.accounts.config;
    require!(!config.redeem_paused, PoyzError::RedeemPaused);
    require!(
        ctx.accounts.keeper_account.active,
        PoyzError::KeeperInactive
    );
    require!(
        ctx.accounts.keeper_account.bonded >= config.min_keeper_bond,
        PoyzError::InsufficientBond
    );
    require!(unwind_proof_hash != [0u8; 32], PoyzError::EmptyProofHash);

    let clock = Clock::get()?;
    let elapsed = clock
        .unix_timestamp
        .checked_sub(ctx.accounts.request.created_at)
        .ok_or(PoyzError::MathOverflow)?;
    require!(
        elapsed >= i64::from(config.min_settlement_delay_sec),
        PoyzError::SettlementDelayActive
    );
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

    let synthetic_amount = ctx.accounts.request.synthetic_amount;
    let fee = mul_bps_ceil(synthetic_amount, config.redeem_fee_bps)?;
    let net_synthetic = synthetic_amount
        .checked_sub(fee)
        .ok_or(PoyzError::MathOverflow)?;

    let collateral_now = notional_to_collateral(
        net_synthetic,
        price.price,
        price.expo,
        config.collateral_decimals,
        config.synthetic_decimals,
    )?;
    let effective_collateral = collateral_now.min(ctx.accounts.request.quoted_collateral);
    require!(effective_collateral > 0, PoyzError::ZeroAmount);
    require!(
        effective_collateral >= ctx.accounts.request.min_collateral_out,
        PoyzError::SlippageExceeded
    );
    require!(
        effective_collateral <= config.total_collateral,
        PoyzError::RedeemExceedsCollateral
    );

    // The short must have been closed for at least the notional leaving the
    // book, within the slippage band.
    let required_unwind = mul_bps_floor(
        synthetic_amount,
        10_000u16
            .checked_sub(config.max_hedge_slippage_bps)
            .ok_or(PoyzError::InvalidBps)?,
    )?;
    require!(
        unwound_notional >= required_unwind,
        PoyzError::HedgeFillTooSmall
    );

    let config_bump = config.bump;
    let seeds: &[&[u8]] = &[CONFIG_SEED, std::slice::from_ref(&config_bump)];
    let signer_seeds: &[&[&[u8]]] = &[seeds];
    let collateral_decimals = ctx.accounts.collateral_mint.decimals;

    // Burn first, then release. If the release were to fail the whole
    // instruction reverts, but ordering the supply reduction ahead of the
    // outflow keeps the intermediate state conservative.
    token_interface::burn(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Burn {
                mint: ctx.accounts.synthetic_mint.to_account_info(),
                from: ctx.accounts.redeem_escrow.to_account_info(),
                authority: ctx.accounts.config.to_account_info(),
            },
            signer_seeds,
        ),
        synthetic_amount,
    )?;

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
        effective_collateral,
        collateral_decimals,
    )?;

    let nonce = ctx.accounts.request.nonce;
    let user_key = ctx.accounts.request.user;
    let keeper_key = ctx.accounts.keeper.key();

    let config = &mut ctx.accounts.config;
    config.pending_redeem_synthetic = config
        .pending_redeem_synthetic
        .checked_sub(synthetic_amount)
        .ok_or(PoyzError::MathOverflow)?;
    config.total_synthetic = config
        .total_synthetic
        .checked_sub(synthetic_amount)
        .ok_or(PoyzError::MathOverflow)?;
    config.total_collateral = config
        .total_collateral
        .checked_sub(effective_collateral)
        .ok_or(PoyzError::MathOverflow)?;
    // Saturating: the keeper may report a larger unwind than the notional this
    // redeem removed (venues close in lots). Under-reporting is caught by the
    // rebalance proof chain, not here.
    config.hedged_notional = config.hedged_notional.saturating_sub(unwound_notional);

    emit!(RedeemConfirmed {
        config: config.key(),
        user: user_key,
        keeper: keeper_key,
        nonce,
        synthetic_burned: synthetic_amount,
        collateral_returned: effective_collateral,
        fee,
        unwound_notional,
        venue_id,
        unwind_proof_hash,
        total_synthetic: config.total_synthetic,
        total_collateral: config.total_collateral,
        hedged_notional: config.hedged_notional,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// redeem_cancel
// ---------------------------------------------------------------------------

#[derive(Accounts)]
#[instruction(nonce: u64)]
pub struct RedeemCancelCtx<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = synthetic_mint @ PoyzError::TokenProgramMismatch,
        constraint = token_program.key() == config.token_program @ PoyzError::TokenProgramMismatch,
    )]
    pub config: Box<Account<'info, Config>>,

    #[account(
        mut,
        close = user,
        seeds = [REDEEM_REQUEST_SEED, user.key().as_ref(), &nonce.to_le_bytes()],
        bump = request.bump,
        has_one = user @ PoyzError::Unauthorized,
    )]
    pub request: Box<Account<'info, RedeemRequest>>,

    pub synthetic_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        seeds = [REDEEM_ESCROW_SEED],
        bump,
        token::mint = synthetic_mint,
        token::authority = config,
        token::token_program = token_program,
    )]
    pub redeem_escrow: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        token::mint = synthetic_mint,
        token::authority = user,
        token::token_program = token_program,
    )]
    pub user_synthetic: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Interface<'info, TokenInterface>,
}

/// Reclaim escrowed synthetic from an expired redeem request. Callable while
/// paused, for the same reason `mint_cancel` is.
pub fn redeem_cancel(ctx: Context<RedeemCancelCtx>, _nonce: u64) -> Result<()> {
    let clock = Clock::get()?;
    require!(
        clock.unix_timestamp > ctx.accounts.request.deadline,
        PoyzError::RequestNotExpired
    );

    let amount = ctx.accounts.request.synthetic_amount;
    let nonce = ctx.accounts.request.nonce;
    let decimals = ctx.accounts.synthetic_mint.decimals;
    let config_bump = ctx.accounts.config.bump;
    let seeds: &[&[u8]] = &[CONFIG_SEED, std::slice::from_ref(&config_bump)];
    let signer_seeds: &[&[&[u8]]] = &[seeds];

    token_interface::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.redeem_escrow.to_account_info(),
                mint: ctx.accounts.synthetic_mint.to_account_info(),
                to: ctx.accounts.user_synthetic.to_account_info(),
                authority: ctx.accounts.config.to_account_info(),
            },
            signer_seeds,
        ),
        amount,
        decimals,
    )?;

    let user_key = ctx.accounts.user.key();
    let config = &mut ctx.accounts.config;
    config.pending_redeem_synthetic = config
        .pending_redeem_synthetic
        .checked_sub(amount)
        .ok_or(PoyzError::MathOverflow)?;

    emit!(RedeemCancelled {
        config: config.key(),
        user: user_key,
        nonce,
        synthetic_returned: amount,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
