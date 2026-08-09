//! Delta Keeper registration, bonding, unbonding and slashing.
//!
//! A keeper is only trustworthy to the extent that lying costs it money. The
//! bond is real $POYZ held in a program-owned vault, and the only instructions
//! that require an active bond are the ones where a keeper asserts something
//! the chain cannot verify by itself: `mint_confirm`, `redeem_confirm` and
//! `commit_rebalance_proof`.
//!
//! Unbonding is gated on `unbond_cooldown_sec` measured from the keeper's last
//! committed proof, not from the unbond request. Evidence that a proof was
//! false surfaces when someone reconciles it against the venue's public trade
//! history, which happens within minutes to hours of the proof. Anchoring the
//! cooldown to the last proof means a keeper cannot commit a false proof and
//! withdraw its bond in the same block.

use anchor_lang::prelude::*;
use anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked};

use crate::errors::PoyzError;
use crate::events::{KeeperBonded, KeeperRegistered, KeeperSlashed, KeeperUnbonded};
use crate::state::*;

/// Keep `Config::keeper_count` in step with a keeper's active flag. Called on
/// every transition so the counter can never double-count a keeper that
/// crosses the minimum bond more than once.
fn set_active(config: &mut Config, keeper: &mut Keeper, active: bool) -> Result<()> {
    if keeper.active == active {
        return Ok(());
    }
    keeper.active = active;
    if active {
        config.keeper_count = config
            .keeper_count
            .checked_add(1)
            .ok_or(PoyzError::MathOverflow)?;
    } else {
        config.keeper_count = config.keeper_count.saturating_sub(1);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// keeper_register
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct KeeperRegister<'info> {
    #[account(mut)]
    pub keeper: Signer<'info>,

    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = bond_mint @ PoyzError::TokenProgramMismatch,
        constraint = token_program.key() == config.token_program @ PoyzError::TokenProgramMismatch,
    )]
    pub config: Box<Account<'info, Config>>,

    #[account(
        init,
        payer = keeper,
        space = 8 + Keeper::LEN,
        seeds = [KEEPER_SEED, keeper.key().as_ref()],
        bump
    )]
    pub keeper_account: Box<Account<'info, Keeper>>,

    pub bond_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        token::mint = bond_mint,
        token::authority = keeper,
        token::token_program = token_program,
    )]
    pub keeper_bond_source: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [BOND_VAULT_SEED],
        bump,
        token::mint = bond_mint,
        token::authority = config,
        token::token_program = token_program,
    )]
    pub bond_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn keeper_register(ctx: Context<KeeperRegister>, bond_amount: u64) -> Result<()> {
    require!(
        bond_amount >= ctx.accounts.config.min_keeper_bond,
        PoyzError::InsufficientBond
    );

    let decimals = ctx.accounts.bond_mint.decimals;
    token_interface::transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.keeper_bond_source.to_account_info(),
                mint: ctx.accounts.bond_mint.to_account_info(),
                to: ctx.accounts.bond_vault.to_account_info(),
                authority: ctx.accounts.keeper.to_account_info(),
            },
        ),
        bond_amount,
        decimals,
    )?;

    let now = Clock::get()?.unix_timestamp;
    let keeper_key = ctx.accounts.keeper.key();

    let keeper_account = &mut ctx.accounts.keeper_account;
    keeper_account.keeper = keeper_key;
    keeper_account.bonded = bond_amount;
    keeper_account.slashed = 0;
    keeper_account.proofs_committed = 0;
    keeper_account.registered_at = now;
    keeper_account.last_proof_at = 0;
    keeper_account.last_proof_slot = 0;
    keeper_account.last_bond_at = now;
    keeper_account.active = false;
    keeper_account.bump = ctx.bumps.keeper_account;
    keeper_account.reserved = [0u8; 14];

    let config = &mut ctx.accounts.config;
    config.bonded_total = config
        .bonded_total
        .checked_add(bond_amount)
        .ok_or(PoyzError::MathOverflow)?;
    set_active(config, keeper_account, true)?;

    emit!(KeeperRegistered {
        config: config.key(),
        keeper: keeper_key,
        bonded: bond_amount,
        keeper_count: config.keeper_count,
        timestamp: now,
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// keeper_bond -- top up an existing bond
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct KeeperBond<'info> {
    #[account(mut)]
    pub keeper: Signer<'info>,

    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = bond_mint @ PoyzError::TokenProgramMismatch,
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

    pub bond_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        token::mint = bond_mint,
        token::authority = keeper,
        token::token_program = token_program,
    )]
    pub keeper_bond_source: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [BOND_VAULT_SEED],
        bump,
        token::mint = bond_mint,
        token::authority = config,
        token::token_program = token_program,
    )]
    pub bond_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn keeper_bond(ctx: Context<KeeperBond>, amount: u64) -> Result<()> {
    require!(amount > 0, PoyzError::ZeroAmount);

    let decimals = ctx.accounts.bond_mint.decimals;
    token_interface::transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.keeper_bond_source.to_account_info(),
                mint: ctx.accounts.bond_mint.to_account_info(),
                to: ctx.accounts.bond_vault.to_account_info(),
                authority: ctx.accounts.keeper.to_account_info(),
            },
        ),
        amount,
        decimals,
    )?;

    let now = Clock::get()?.unix_timestamp;
    let min_bond = ctx.accounts.config.min_keeper_bond;

    let keeper_account = &mut ctx.accounts.keeper_account;
    keeper_account.bonded = keeper_account
        .bonded
        .checked_add(amount)
        .ok_or(PoyzError::MathOverflow)?;
    keeper_account.last_bond_at = now;
    let bonded = keeper_account.bonded;

    let config = &mut ctx.accounts.config;
    config.bonded_total = config
        .bonded_total
        .checked_add(amount)
        .ok_or(PoyzError::MathOverflow)?;
    // Topping back above the minimum re-activates a keeper that a slash
    // knocked out.
    set_active(config, keeper_account, bonded >= min_bond)?;

    emit!(KeeperBonded {
        config: config.key(),
        keeper: keeper_account.keeper,
        added: amount,
        bonded,
        active: keeper_account.active,
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// keeper_unbond
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct KeeperUnbond<'info> {
    pub keeper: Signer<'info>,

    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = bond_mint @ PoyzError::TokenProgramMismatch,
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

    pub bond_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        seeds = [BOND_VAULT_SEED],
        bump,
        token::mint = bond_mint,
        token::authority = config,
        token::token_program = token_program,
    )]
    pub bond_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        token::mint = bond_mint,
        token::authority = keeper,
        token::token_program = token_program,
    )]
    pub keeper_bond_destination: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn keeper_unbond(ctx: Context<KeeperUnbond>, amount: u64) -> Result<()> {
    require!(amount > 0, PoyzError::ZeroAmount);
    require!(
        amount <= ctx.accounts.keeper_account.bonded,
        PoyzError::SlashExceedsBond
    );

    let now = Clock::get()?.unix_timestamp;
    let cooldown = i64::from(ctx.accounts.config.unbond_cooldown_sec);
    let last_proof_at = ctx.accounts.keeper_account.last_proof_at;
    if last_proof_at > 0 {
        let elapsed = now
            .checked_sub(last_proof_at)
            .ok_or(PoyzError::MathOverflow)?;
        require!(elapsed >= cooldown, PoyzError::UnbondCooldownActive);
    }

    let min_bond = ctx.accounts.config.min_keeper_bond;
    let remaining = ctx
        .accounts
        .keeper_account
        .bonded
        .checked_sub(amount)
        .ok_or(PoyzError::MathOverflow)?;
    // Either stay fully bonded, or exit completely. A keeper lingering with a
    // dust bond would still show up in `keeper_count` semantics elsewhere.
    require!(
        remaining == 0 || remaining >= min_bond,
        PoyzError::BondBelowMinimum
    );

    let decimals = ctx.accounts.bond_mint.decimals;
    let config_bump = ctx.accounts.config.bump;
    let seeds: &[&[u8]] = &[CONFIG_SEED, std::slice::from_ref(&config_bump)];
    let signer_seeds: &[&[&[u8]]] = &[seeds];

    token_interface::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.bond_vault.to_account_info(),
                mint: ctx.accounts.bond_mint.to_account_info(),
                to: ctx.accounts.keeper_bond_destination.to_account_info(),
                authority: ctx.accounts.config.to_account_info(),
            },
            signer_seeds,
        ),
        amount,
        decimals,
    )?;

    let keeper_account = &mut ctx.accounts.keeper_account;
    keeper_account.bonded = remaining;

    let config = &mut ctx.accounts.config;
    config.bonded_total = config
        .bonded_total
        .checked_sub(amount)
        .ok_or(PoyzError::MathOverflow)?;
    set_active(config, keeper_account, remaining >= min_bond)?;

    emit!(KeeperUnbonded {
        config: config.key(),
        keeper: keeper_account.keeper,
        withdrawn: amount,
        bonded: remaining,
        active: keeper_account.active,
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// keeper_slash
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct KeeperSlash<'info> {
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

    #[account(
        mut,
        seeds = [KEEPER_SEED, keeper_account.keeper.as_ref()],
        bump = keeper_account.bump,
    )]
    pub keeper_account: Box<Account<'info, Keeper>>,

    pub bond_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        seeds = [BOND_VAULT_SEED],
        bump,
        token::mint = bond_mint,
        token::authority = config,
        token::token_program = token_program,
    )]
    pub bond_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [BUFFER_BOND_VAULT_SEED],
        bump,
        token::mint = bond_mint,
        token::authority = config,
        token::token_program = token_program,
    )]
    pub buffer_bond_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Interface<'info, TokenInterface>,
}

/// Slash a keeper's bond.
///
/// The *decision* is off-chain: reconciling a committed `proof_hash` against
/// the venue's public execution history, or observing that a delta breach went
/// unaddressed past the rebalance window, cannot be done inside a Solana
/// instruction. What is on-chain is the consequence and its justification:
/// `reason_code` and `evidence_hash` are recorded in the event so the evidence
/// bundle that produced the slash is committed to and can be published and
/// checked by anyone afterwards. A slash with an evidence hash nobody can
/// reproduce is itself visible misconduct by the authority.
///
/// Slashed bond moves to the insurance buffer's $POYZ vault. Converting it into
/// synthetic dollars requires a market sale and is a governance action, not an
/// on-chain one -- the program does not embed a swap route it cannot verify.
pub fn keeper_slash(
    ctx: Context<KeeperSlash>,
    amount: u64,
    reason_code: u8,
    evidence_hash: [u8; 32],
) -> Result<()> {
    require!(amount > 0, PoyzError::ZeroAmount);
    require!(
        amount <= ctx.accounts.keeper_account.bonded,
        PoyzError::SlashExceedsBond
    );
    require!(evidence_hash != [0u8; 32], PoyzError::EmptyProofHash);
    // Rule-based, not discretionary: the authority must name which enumerated
    // fault was committed (`state::SLASH_REASON_*`, from `docs/security.md`
    // 2.4). A slash that cannot be attributed to a published rule is not a
    // slash the protocol will record.
    require!(
        reason_code >= SLASH_REASON_DELTA_OUT_OF_BAND && reason_code <= SLASH_REASON_MAX,
        PoyzError::UnknownSlashReason
    );

    let decimals = ctx.accounts.bond_mint.decimals;
    let config_bump = ctx.accounts.config.bump;
    let seeds: &[&[u8]] = &[CONFIG_SEED, std::slice::from_ref(&config_bump)];
    let signer_seeds: &[&[&[u8]]] = &[seeds];

    token_interface::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.bond_vault.to_account_info(),
                mint: ctx.accounts.bond_mint.to_account_info(),
                to: ctx.accounts.buffer_bond_vault.to_account_info(),
                authority: ctx.accounts.config.to_account_info(),
            },
            signer_seeds,
        ),
        amount,
        decimals,
    )?;

    let min_bond = ctx.accounts.config.min_keeper_bond;

    let keeper_account = &mut ctx.accounts.keeper_account;
    keeper_account.bonded = keeper_account
        .bonded
        .checked_sub(amount)
        .ok_or(PoyzError::MathOverflow)?;
    keeper_account.slashed = keeper_account
        .slashed
        .checked_add(amount)
        .ok_or(PoyzError::MathOverflow)?;
    let bonded = keeper_account.bonded;

    let config = &mut ctx.accounts.config;
    config.bonded_total = config
        .bonded_total
        .checked_sub(amount)
        .ok_or(PoyzError::MathOverflow)?;
    config.slashed_total = config
        .slashed_total
        .checked_add(amount)
        .ok_or(PoyzError::MathOverflow)?;
    set_active(config, keeper_account, bonded >= min_bond)?;

    emit!(KeeperSlashed {
        config: config.key(),
        keeper: keeper_account.keeper,
        slashed: amount,
        bonded,
        active: keeper_account.active,
        reason_code,
        evidence_hash,
    });

    Ok(())
}
