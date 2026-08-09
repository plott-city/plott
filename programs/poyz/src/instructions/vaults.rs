//! Creation of the program-owned token accounts.
//!
//! Split into four instructions, two `init`s at most per instruction. Anchor's
//! `init` for a token account expands into a large amount of stack-resident
//! CPI setup; packing all seven vaults into one instruction reproduces the
//! "Stack offset exceeded max offset" failure documented in
//! `references/solana/anchor-lessons.md`. The split also keeps each transaction
//! comfortably inside the account limit.
//!
//! Every vault is a seeded PDA token account whose authority is the config PDA.
//! Seeded PDAs rather than associated token accounts: the protocol needs four
//! distinct accounts of the *same* mint (funding, buffer, stake, redeem
//! escrow) under the *same* authority, and only one associated token account
//! can exist per (mint, owner) pair.

use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::errors::PoyzError;
use crate::events::VaultGroupInitialized;
use crate::state::*;

fn mark_initialized(config: &mut Config, config_key: Pubkey, flag: u8) -> Result<()> {
    require!(
        config.vault_flags & flag == 0,
        PoyzError::VaultAlreadyInitialized
    );
    config.vault_flags |= flag;

    emit!(VaultGroupInitialized {
        config: config_key,
        flag,
        vault_flags: config.vault_flags,
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// collateral vault
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct InitCollateralVault<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = authority @ PoyzError::Unauthorized,
        has_one = collateral_mint @ PoyzError::TokenProgramMismatch,
        constraint = token_program.key() == config.token_program @ PoyzError::TokenProgramMismatch,
    )]
    pub config: Box<Account<'info, Config>>,

    pub collateral_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        init,
        payer = authority,
        seeds = [COLLATERAL_VAULT_SEED, collateral_mint.key().as_ref()],
        bump,
        token::mint = collateral_mint,
        token::authority = config,
        token::token_program = token_program,
    )]
    pub collateral_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn init_collateral_vault(ctx: Context<InitCollateralVault>) -> Result<()> {
    let config_key = ctx.accounts.config.key();
    mark_initialized(&mut ctx.accounts.config, config_key, VAULT_FLAG_COLLATERAL)
}

// ---------------------------------------------------------------------------
// bond vaults ($POYZ): live bonds, and slashed bonds held by the buffer
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct InitBondVaults<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = authority @ PoyzError::Unauthorized,
        has_one = bond_mint @ PoyzError::TokenProgramMismatch,
        constraint = token_program.key() == config.token_program @ PoyzError::TokenProgramMismatch,
    )]
    pub config: Box<Account<'info, Config>>,

    pub bond_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        init,
        payer = authority,
        seeds = [BOND_VAULT_SEED],
        bump,
        token::mint = bond_mint,
        token::authority = config,
        token::token_program = token_program,
    )]
    pub bond_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Slashed keeper bonds land here. Held separately from live bonds so the
    /// live-bond vault balance always equals the sum of `Keeper::bonded`, which
    /// makes an accounting drift detectable by an external observer.
    #[account(
        init,
        payer = authority,
        seeds = [BUFFER_BOND_VAULT_SEED],
        bump,
        token::mint = bond_mint,
        token::authority = config,
        token::token_program = token_program,
    )]
    pub buffer_bond_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn init_bond_vaults(ctx: Context<InitBondVaults>) -> Result<()> {
    let config_key = ctx.accounts.config.key();
    mark_initialized(&mut ctx.accounts.config, config_key, VAULT_FLAG_BOND)
}

// ---------------------------------------------------------------------------
// funding + insurance buffer vaults (synthetic dollar)
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct InitFundingVaults<'info> {
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
        init,
        payer = authority,
        seeds = [FUNDING_VAULT_SEED],
        bump,
        token::mint = synthetic_mint,
        token::authority = config,
        token::token_program = token_program,
    )]
    pub funding_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init,
        payer = authority,
        seeds = [BUFFER_VAULT_SEED],
        bump,
        token::mint = synthetic_mint,
        token::authority = config,
        token::token_program = token_program,
    )]
    pub buffer_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn init_funding_vaults(ctx: Context<InitFundingVaults>) -> Result<()> {
    let config_key = ctx.accounts.config.key();
    mark_initialized(&mut ctx.accounts.config, config_key, VAULT_FLAG_FUNDING)
}

// ---------------------------------------------------------------------------
// stake vault + redeem escrow (synthetic dollar)
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct InitStakeVaults<'info> {
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
        init,
        payer = authority,
        seeds = [STAKE_VAULT_SEED],
        bump,
        token::mint = synthetic_mint,
        token::authority = config,
        token::token_program = token_program,
    )]
    pub stake_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Redeem requests escrow their synthetic dollars here rather than burning
    /// on request. Burning early and re-minting on cancel would put a mint CPI
    /// on the cancel path -- the one path a user can trigger unilaterally --
    /// and any bug there is unbacked issuance. Escrow makes cancel a transfer.
    #[account(
        init,
        payer = authority,
        seeds = [REDEEM_ESCROW_SEED],
        bump,
        token::mint = synthetic_mint,
        token::authority = config,
        token::token_program = token_program,
    )]
    pub redeem_escrow: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn init_stake_vaults(ctx: Context<InitStakeVaults>) -> Result<()> {
    let config_key = ctx.accounts.config.key();
    mark_initialized(&mut ctx.accounts.config, config_key, VAULT_FLAG_STAKE)
}
