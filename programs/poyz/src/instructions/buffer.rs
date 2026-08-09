//! Insurance buffer.
//!
//! The buffer exists for one scenario the product is honest about: perpetual
//! funding can go negative and stay negative. In that regime the protocol pays
//! to keep the hedge on, and staker yield goes to zero or below. The buffer is
//! the reserve that absorbs it.
//!
//! # The withdrawal constraint, and why it is structural rather than a policy
//!
//! `buffer_withdraw` has no destination argument. The destination is the
//! funding vault, pinned by PDA seeds in the account context. The authority
//! chooses *whether* and *how much*, never *where*. So even a fully compromised
//! authority cannot move the insurance buffer to an address it controls -- the
//! worst it can do is accelerate a distribution to stakers, which is what the
//! buffer is for.
//!
//! That is deliberately stronger than "the authority is a multisig". A multisig
//! narrows who can sign; this narrows what a signature can express. On top of
//! it sit three further gates:
//!
//!   * funding must currently be in a negative regime
//!     (`negative_funding_since != 0`, set by `settle_funding`);
//!   * that regime must have persisted for `buffer_unlock_delay_sec`, so a
//!     single negative print cannot unlock the reserve;
//!   * a single call may draw at most `buffer_max_draw_bps` of the balance.
//!
//! Deposits are permissionless. Anyone may strengthen the buffer -- the
//! protocol treasury, a DAO, or a large holder with an interest in the peg.

use anchor_lang::prelude::*;
use anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked};

use crate::errors::PoyzError;
use crate::events::{BufferDeposited, BufferWithdrawn};
use crate::math::{acc_increment, mul_bps_floor};
use crate::state::*;

// ---------------------------------------------------------------------------
// buffer_deposit
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct BufferDeposit<'info> {
    #[account(mut)]
    pub depositor: Signer<'info>,

    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = synthetic_mint @ PoyzError::TokenProgramMismatch,
        constraint = token_program.key() == config.token_program @ PoyzError::TokenProgramMismatch,
    )]
    pub config: Box<Account<'info, Config>>,

    pub synthetic_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        token::mint = synthetic_mint,
        token::authority = depositor,
        token::token_program = token_program,
    )]
    pub depositor_synthetic: Box<InterfaceAccount<'info, TokenAccount>>,

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

pub fn buffer_deposit(ctx: Context<BufferDeposit>, amount: u64) -> Result<()> {
    require!(amount > 0, PoyzError::ZeroAmount);

    let decimals = ctx.accounts.synthetic_mint.decimals;
    token_interface::transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.depositor_synthetic.to_account_info(),
                mint: ctx.accounts.synthetic_mint.to_account_info(),
                to: ctx.accounts.buffer_vault.to_account_info(),
                authority: ctx.accounts.depositor.to_account_info(),
            },
        ),
        amount,
        decimals,
    )?;

    let depositor_key = ctx.accounts.depositor.key();
    let config = &mut ctx.accounts.config;
    config.buffer_balance = config
        .buffer_balance
        .checked_add(amount)
        .ok_or(PoyzError::MathOverflow)?;

    emit!(BufferDeposited {
        config: config.key(),
        depositor: depositor_key,
        amount,
        buffer_balance: config.buffer_balance,
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// buffer_withdraw
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct BufferWithdraw<'info> {
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
        seeds = [BUFFER_VAULT_SEED],
        bump,
        token::mint = synthetic_mint,
        token::authority = config,
        token::token_program = token_program,
    )]
    pub buffer_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The only possible destination. Pinned by seeds, not passed in by the
    /// caller: the authority decides whether and how much, never where.
    #[account(
        mut,
        seeds = [FUNDING_VAULT_SEED],
        bump,
        token::mint = synthetic_mint,
        token::authority = config,
        token::token_program = token_program,
    )]
    pub funding_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn buffer_withdraw(ctx: Context<BufferWithdraw>, amount: u64) -> Result<()> {
    let config = &ctx.accounts.config;
    require!(amount > 0, PoyzError::ZeroAmount);
    require!(
        amount <= config.buffer_balance,
        PoyzError::InsufficientBuffer
    );
    // The draw is distributed per staked unit, so there has to be a
    // denominator.
    require!(config.total_staked > 0, PoyzError::NoStakers);

    require!(config.negative_funding_since != 0, PoyzError::BufferLocked);
    let now = Clock::get()?.unix_timestamp;
    let in_regime = now
        .checked_sub(config.negative_funding_since)
        .ok_or(PoyzError::MathOverflow)?;
    require!(
        in_regime >= i64::from(config.buffer_unlock_delay_sec),
        PoyzError::BufferLocked
    );

    let draw_cap = mul_bps_floor(config.buffer_balance, config.buffer_max_draw_bps)?;
    require!(amount <= draw_cap, PoyzError::BufferDrawCapExceeded);

    let total_staked = config.total_staked;
    let decimals = ctx.accounts.synthetic_mint.decimals;
    let config_bump = config.bump;
    let seeds: &[&[u8]] = &[CONFIG_SEED, std::slice::from_ref(&config_bump)];
    let signer_seeds: &[&[&[u8]]] = &[seeds];

    token_interface::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.buffer_vault.to_account_info(),
                mint: ctx.accounts.synthetic_mint.to_account_info(),
                to: ctx.accounts.funding_vault.to_account_info(),
                authority: ctx.accounts.config.to_account_info(),
            },
            signer_seeds,
        ),
        amount,
        decimals,
    )?;

    let authority_key = ctx.accounts.authority.key();
    let config = &mut ctx.accounts.config;
    config.buffer_balance = config
        .buffer_balance
        .checked_sub(amount)
        .ok_or(PoyzError::MathOverflow)?;
    config.staker_funding_balance = config
        .staker_funding_balance
        .checked_add(amount)
        .ok_or(PoyzError::MathOverflow)?;
    config.acc_funding_per_share = config
        .acc_funding_per_share
        .checked_add(acc_increment(amount, total_staked)?)
        .ok_or(PoyzError::MathOverflow)?;

    emit!(BufferWithdrawn {
        config: config.key(),
        authority: authority_key,
        amount,
        buffer_balance: config.buffer_balance,
        acc_funding_per_share: config.acc_funding_per_share,
        negative_funding_since: config.negative_funding_since,
        timestamp: now,
    });

    Ok(())
}
