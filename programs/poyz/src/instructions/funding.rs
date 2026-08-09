//! Funding revenue: settlement, staking, and pro-rata claims.
//!
//! # Accounting model
//!
//! A single accumulator, `Config::acc_funding_per_share`, scaled by
//! `math::ACC_SCALE`. Each settlement adds `amount * SCALE / total_staked` to
//! it. A staker's lifetime entitlement is `amount * acc / SCALE`, and
//! `StakePosition::reward_debt` records how much of that entitlement has
//! already been accounted for. The difference is what they may claim.
//!
//! The alternative -- iterating over stakers at settlement time -- is not
//! implementable on Solana at any realistic staker count. The accumulator makes
//! settlement O(1) and claiming O(1), and it is the reason `amount` may never
//! change without first moving the outstanding difference into `unclaimed`:
//! raising `amount` first would retroactively apply the whole historical
//! accumulator to newly staked tokens, paying a staker for funding that accrued
//! before they arrived.
//!
//! # Denomination
//!
//! Funding is settled in the synthetic dollar. The venue pays funding in its
//! own quote asset off-chain; the treasury converts and brings the proceeds
//! on-chain as synthetic dollars. Keeping the funding vault, the insurance
//! buffer and the stake vault all in one unit means the distribution math never
//! needs a second oracle, and a staker's claim is denominated in the same
//! instrument they staked.
//!
//! # Negative funding
//!
//! The carry regime is *not* written here. `report_venue_state` is the single
//! writer of `last_net_carry_bps` and `negative_funding_since`, so there is
//! exactly one place the insurance-buffer precondition can come from. A
//! settlement moves tokens; observing the market is a separate act, and it
//! must be possible to record a negative window in which no tokens moved at
//! all.

use anchor_lang::prelude::*;
use anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked};

use crate::errors::PoyzError;
use crate::events::{FundingClaimed, FundingSettled, Staked, UnstakeRequested, Unstaked};
use crate::math::{acc_entitlement, acc_increment, mul_bps_ceil};
use crate::state::*;

/// Move everything the position has earned since its last touch into
/// `unclaimed`, then re-baseline `reward_debt`. Must run before any change to
/// `StakePosition::amount`.
fn accrue(position: &mut StakePosition, acc_funding_per_share: u128) -> Result<()> {
    let entitlement = acc_entitlement(position.amount, acc_funding_per_share)?;
    let pending = entitlement
        .checked_sub(position.reward_debt)
        .ok_or(PoyzError::MathOverflow)?;
    if pending > 0 {
        let pending_u64 = u64::try_from(pending).map_err(|_| error!(PoyzError::MathOverflow))?;
        position.unclaimed = position
            .unclaimed
            .checked_add(pending_u64)
            .ok_or(PoyzError::MathOverflow)?;
    }
    position.reward_debt = entitlement;
    Ok(())
}

// ---------------------------------------------------------------------------
// settle_funding
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct SettleFunding<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = authority @ PoyzError::Unauthorized,
        has_one = synthetic_mint @ PoyzError::TokenProgramMismatch,
        constraint = token_program.key() == config.token_program @ PoyzError::TokenProgramMismatch,
    )]
    pub config: Box<Account<'info, Config>>,

    pub synthetic_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        token::mint = synthetic_mint,
        token::authority = authority,
        token::token_program = token_program,
    )]
    pub authority_synthetic: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [FUNDING_VAULT_SEED],
        bump,
        token::mint = synthetic_mint,
        token::authority = config,
        token::token_program = token_program,
    )]
    pub funding_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [BUFFER_VAULT_SEED],
        bump,
        token::mint = synthetic_mint,
        token::authority = config,
        token::token_program = token_program,
    )]
    pub buffer_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Interface<'info, TokenInterface>,
}

/// Book a funding settlement.
///
/// Deliberately callable while paused: a settlement is an inflow, and a pause
/// exists to stop issuance, not to stop the protocol being paid.
pub fn settle_funding(ctx: Context<SettleFunding>, amount: u64) -> Result<()> {
    let config = &ctx.accounts.config;
    let total_staked = config.total_staked;

    // With nothing staked there is no per-share denominator, so the entire
    // settlement goes to the insurance buffer instead of being stranded.
    let to_buffer = if total_staked == 0 {
        amount
    } else {
        // Ceil toward the buffer: the sub-unit residue accrues to protocol
        // equity rather than to whichever staker claims first.
        mul_bps_ceil(amount, config.buffer_share_bps)?
    };
    let to_stakers = amount
        .checked_sub(to_buffer)
        .ok_or(PoyzError::MathOverflow)?;

    let decimals = ctx.accounts.synthetic_mint.decimals;

    if to_buffer > 0 {
        token_interface::transfer_checked(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.authority_synthetic.to_account_info(),
                    mint: ctx.accounts.synthetic_mint.to_account_info(),
                    to: ctx.accounts.buffer_vault.to_account_info(),
                    authority: ctx.accounts.authority.to_account_info(),
                },
            ),
            to_buffer,
            decimals,
        )?;
    }

    if to_stakers > 0 {
        token_interface::transfer_checked(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.authority_synthetic.to_account_info(),
                    mint: ctx.accounts.synthetic_mint.to_account_info(),
                    to: ctx.accounts.funding_vault.to_account_info(),
                    authority: ctx.accounts.authority.to_account_info(),
                },
            ),
            to_stakers,
            decimals,
        )?;
    }

    let now = Clock::get()?.unix_timestamp;
    let authority_key = ctx.accounts.authority.key();

    let config = &mut ctx.accounts.config;
    if to_stakers > 0 {
        config.acc_funding_per_share = config
            .acc_funding_per_share
            .checked_add(acc_increment(to_stakers, total_staked)?)
            .ok_or(PoyzError::MathOverflow)?;
        config.staker_funding_balance = config
            .staker_funding_balance
            .checked_add(to_stakers)
            .ok_or(PoyzError::MathOverflow)?;
    }
    if to_buffer > 0 {
        config.buffer_balance = config
            .buffer_balance
            .checked_add(to_buffer)
            .ok_or(PoyzError::MathOverflow)?;
    }

    config.last_settle_at = now;

    emit!(FundingSettled {
        config: config.key(),
        authority: authority_key,
        amount,
        to_stakers,
        to_buffer,
        net_carry_bps: config.last_net_carry_bps,
        acc_funding_per_share: config.acc_funding_per_share,
        total_staked,
        negative_funding_since: config.negative_funding_since,
        timestamp: now,
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// stake
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = synthetic_mint @ PoyzError::TokenProgramMismatch,
        constraint = token_program.key() == config.token_program @ PoyzError::TokenProgramMismatch,
    )]
    pub config: Box<Account<'info, Config>>,

    #[account(
        init_if_needed,
        payer = owner,
        space = 8 + StakePosition::LEN,
        seeds = [STAKE_POSITION_SEED, owner.key().as_ref()],
        bump
    )]
    pub position: Box<Account<'info, StakePosition>>,

    pub synthetic_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        token::mint = synthetic_mint,
        token::authority = owner,
        token::token_program = token_program,
    )]
    pub owner_synthetic: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [STAKE_VAULT_SEED],
        bump,
        token::mint = synthetic_mint,
        token::authority = config,
        token::token_program = token_program,
    )]
    pub stake_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn stake(ctx: Context<Stake>, amount: u64) -> Result<()> {
    // Gated on the mint breaker: staking is an issuance-side yield election,
    // and a protocol that has stopped issuing should not be taking on new
    // stake either. Unstaking is deliberately never gated.
    require!(!ctx.accounts.config.mint_paused, PoyzError::MintPaused);
    require!(
        ctx.accounts.config.vaults_ready(),
        PoyzError::VaultsNotReady
    );
    require!(amount > 0, PoyzError::ZeroAmount);

    let owner_key = ctx.accounts.owner.key();
    let acc = ctx.accounts.config.acc_funding_per_share;
    let now = Clock::get()?.unix_timestamp;

    {
        let position = &mut ctx.accounts.position;
        // `init_if_needed` leaves a brand-new account zeroed, so an unset
        // owner is how a first-time stake is recognised.
        if position.owner == Pubkey::default() {
            position.owner = owner_key;
            position.bump = ctx.bumps.position;
            position.reserved = [0u8; 7];
        } else {
            require_keys_eq!(position.owner, owner_key, PoyzError::Unauthorized);
        }
        accrue(position, acc)?;
    }

    let decimals = ctx.accounts.synthetic_mint.decimals;
    token_interface::transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.owner_synthetic.to_account_info(),
                mint: ctx.accounts.synthetic_mint.to_account_info(),
                to: ctx.accounts.stake_vault.to_account_info(),
                authority: ctx.accounts.owner.to_account_info(),
            },
        ),
        amount,
        decimals,
    )?;

    let position = &mut ctx.accounts.position;
    position.amount = position
        .amount
        .checked_add(amount)
        .ok_or(PoyzError::MathOverflow)?;
    position.reward_debt = acc_entitlement(position.amount, acc)?;
    position.last_update = now;
    let position_amount = position.amount;

    let config = &mut ctx.accounts.config;
    config.total_staked = config
        .total_staked
        .checked_add(amount)
        .ok_or(PoyzError::MathOverflow)?;

    emit!(Staked {
        config: config.key(),
        owner: owner_key,
        amount,
        position_amount,
        total_staked: config.total_staked,
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// request_unstake -> unstake (two-step, with cooldown)
// ---------------------------------------------------------------------------

/// Accounts shared by both unstake steps.
#[derive(Accounts)]
pub struct UnstakeCtx<'info> {
    pub owner: Signer<'info>,

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
        seeds = [STAKE_POSITION_SEED, owner.key().as_ref()],
        bump = position.bump,
        has_one = owner @ PoyzError::Unauthorized,
    )]
    pub position: Box<Account<'info, StakePosition>>,

    pub synthetic_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        seeds = [STAKE_VAULT_SEED],
        bump,
        token::mint = synthetic_mint,
        token::authority = config,
        token::token_program = token_program,
    )]
    pub stake_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        token::mint = synthetic_mint,
        token::authority = owner,
        token::token_program = token_program,
    )]
    pub owner_synthetic: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Interface<'info, TokenInterface>,
}

/// Begin an exit: stop earning now, withdraw after the cooldown.
///
/// Why a cooldown exists at all. Funding arrives in discrete settlements, so
/// without one the profitable strategy is to stake immediately before a
/// settlement and leave immediately after, collecting a full pro-rata share
/// while bearing none of the risk that produced it -- and diluting everyone who
/// held the position through the period. The cooldown makes that trade cost
/// `unstake_cooldown_sec` of exposure, which is the risk the funding was paying
/// for in the first place.
///
/// The amount stops earning at request time (`total_staked` drops immediately),
/// so waiting out the cooldown is never itself rewarded.
///
/// Never gated on a pause: a pause must not trap principal.
pub fn request_unstake(ctx: Context<UnstakeCtx>, amount: u64) -> Result<()> {
    require!(amount > 0, PoyzError::ZeroAmount);
    require!(
        amount <= ctx.accounts.position.amount,
        PoyzError::InsufficientStake
    );

    let acc = ctx.accounts.config.acc_funding_per_share;
    let cooldown = i64::from(ctx.accounts.config.unstake_cooldown_sec);
    let now = Clock::get()?.unix_timestamp;
    let cooldown_end = now.checked_add(cooldown).ok_or(PoyzError::MathOverflow)?;

    accrue(&mut ctx.accounts.position, acc)?;

    let owner_key = ctx.accounts.owner.key();
    let position = &mut ctx.accounts.position;
    position.amount = position
        .amount
        .checked_sub(amount)
        .ok_or(PoyzError::MathOverflow)?;
    position.pending_unstake = position
        .pending_unstake
        .checked_add(amount)
        .ok_or(PoyzError::MathOverflow)?;
    // A second request restarts the clock for the whole pending balance. The
    // alternative -- tracking a queue of tranches -- costs an unbounded account
    // for no benefit the staker cannot get by simply not requesting again.
    position.cooldown_end = cooldown_end;
    position.reward_debt = acc_entitlement(position.amount, acc)?;
    position.last_update = now;
    let position_amount = position.amount;
    let pending_unstake = position.pending_unstake;

    let config = &mut ctx.accounts.config;
    config.total_staked = config
        .total_staked
        .checked_sub(amount)
        .ok_or(PoyzError::MathOverflow)?;

    emit!(UnstakeRequested {
        config: config.key(),
        owner: owner_key,
        amount,
        position_amount,
        pending_unstake,
        cooldown_end,
        total_staked: config.total_staked,
    });

    Ok(())
}

/// Withdraw the whole pending balance once its cooldown has elapsed.
pub fn unstake(ctx: Context<UnstakeCtx>) -> Result<()> {
    let amount = ctx.accounts.position.pending_unstake;
    require!(amount > 0, PoyzError::NoPendingUnstake);

    let now = Clock::get()?.unix_timestamp;
    require!(
        now >= ctx.accounts.position.cooldown_end,
        PoyzError::UnstakeCooldownActive
    );

    let decimals = ctx.accounts.synthetic_mint.decimals;
    let config_bump = ctx.accounts.config.bump;
    let seeds: &[&[u8]] = &[CONFIG_SEED, std::slice::from_ref(&config_bump)];
    let signer_seeds: &[&[&[u8]]] = &[seeds];

    token_interface::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.stake_vault.to_account_info(),
                mint: ctx.accounts.synthetic_mint.to_account_info(),
                to: ctx.accounts.owner_synthetic.to_account_info(),
                authority: ctx.accounts.config.to_account_info(),
            },
            signer_seeds,
        ),
        amount,
        decimals,
    )?;

    let owner_key = ctx.accounts.owner.key();
    let position = &mut ctx.accounts.position;
    position.pending_unstake = 0;
    position.cooldown_end = 0;
    position.last_update = now;
    let position_amount = position.amount;

    let config = &ctx.accounts.config;

    emit!(Unstaked {
        config: config.key(),
        owner: owner_key,
        amount,
        position_amount,
        total_staked: config.total_staked,
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// claim_funding
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct ClaimFunding<'info> {
    pub owner: Signer<'info>,

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
        seeds = [STAKE_POSITION_SEED, owner.key().as_ref()],
        bump = position.bump,
        has_one = owner @ PoyzError::Unauthorized,
    )]
    pub position: Box<Account<'info, StakePosition>>,

    pub synthetic_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        seeds = [FUNDING_VAULT_SEED],
        bump,
        token::mint = synthetic_mint,
        token::authority = config,
        token::token_program = token_program,
    )]
    pub funding_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        token::mint = synthetic_mint,
        token::authority = owner,
        token::token_program = token_program,
    )]
    pub owner_synthetic: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn claim_funding(ctx: Context<ClaimFunding>) -> Result<()> {
    let acc = ctx.accounts.config.acc_funding_per_share;
    accrue(&mut ctx.accounts.position, acc)?;

    let amount = ctx.accounts.position.unclaimed;
    require!(amount > 0, PoyzError::NothingToClaim);
    // The vault only holds what settlements routed to stakers; a claim larger
    // than that would be paying out of the insurance buffer's share.
    require!(
        amount <= ctx.accounts.config.staker_funding_balance,
        PoyzError::MathOverflow
    );

    let decimals = ctx.accounts.synthetic_mint.decimals;
    let config_bump = ctx.accounts.config.bump;
    let seeds: &[&[u8]] = &[CONFIG_SEED, std::slice::from_ref(&config_bump)];
    let signer_seeds: &[&[&[u8]]] = &[seeds];

    token_interface::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.funding_vault.to_account_info(),
                mint: ctx.accounts.synthetic_mint.to_account_info(),
                to: ctx.accounts.owner_synthetic.to_account_info(),
                authority: ctx.accounts.config.to_account_info(),
            },
            signer_seeds,
        ),
        amount,
        decimals,
    )?;

    let owner_key = ctx.accounts.owner.key();
    let position = &mut ctx.accounts.position;
    position.unclaimed = 0;
    position.claimed_total = position
        .claimed_total
        .checked_add(amount)
        .ok_or(PoyzError::MathOverflow)?;

    let config = &mut ctx.accounts.config;
    config.staker_funding_balance = config
        .staker_funding_balance
        .checked_sub(amount)
        .ok_or(PoyzError::MathOverflow)?;

    emit!(FundingClaimed {
        config: config.key(),
        owner: owner_key,
        amount,
        claimed_total: position.claimed_total,
        staker_funding_balance: config.staker_funding_balance,
    });

    Ok(())
}
