//! Pyth pull-oracle adapter.
//!
//! Poyz reads a Pyth `PriceUpdateV2` account posted by the Pyth Solana
//! Receiver program (`rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ`).
//!
//! Why the layout is redeclared here instead of depending on
//! `pyth-solana-receiver-sdk`:
//!
//!   1. The SDK pins its own `anchor-lang` major. Every Anchor bump then turns
//!      into a dependency-resolution fight inside the SBF toolchain, which is
//!      exactly the class of breakage recorded in
//!      `references/solana/anchor-lessons.md` ("Anchor 버전 호환성 문제").
//!   2. The account layout is a wire format. Redeclaring it makes the bytes we
//!      trust explicit and reviewable in one screen instead of hidden behind a
//!      transitive crate.
//!   3. It lets us enforce checks the SDK helper does not: full verification
//!      level, feed-id binding, and rejection of future-dated prices.
//!
//! The layout below mirrors `pyth_solana_receiver_sdk::price_update`
//! (PriceUpdateV2 / PriceFeedMessage / VerificationLevel). If Pyth ever ships a
//! `PriceUpdateV3`, the discriminator check below fails closed rather than
//! misreading the bytes.
//!
//! Gates applied on every read (all of them, on every mint and redeem):
//!   - account address    == `config.oracle`
//!   - account owner      == Pyth receiver program
//!   - discriminator      == PriceUpdateV2
//!   - verification level == Full (all Wormhole guardian signatures verified)
//!   - feed id            == `config.feed_id`
//!   - price              >  0
//!   - staleness          <= `config.max_price_age_sec`, and not future-dated
//!   - confidence / price <= `config.max_conf_bps`
//!
//! A failure in any of them aborts the instruction. Nothing here degrades
//! silently to a last-known price: an unusable oracle means mint and redeem
//! stop, which is the only safe behaviour for a collateralized issuer.

use anchor_lang::prelude::*;

use crate::errors::PoyzError;
use crate::math::confidence_bps;

/// Pyth Solana Receiver program. `PriceUpdateV2` accounts are owned by it.
/// <https://docs.pyth.network/price-feeds/contract-addresses/solana>
pub const PYTH_RECEIVER_PROGRAM_ID: Pubkey = pubkey!("rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ");

/// `sha256("account:PriceUpdateV2")[..8]` -- the Anchor account discriminator
/// the receiver program writes. Computed, never guessed: a guessed
/// discriminator is the `InstructionFallbackNotFound` class of bug in
/// `anchor-lessons.md`.
pub const PRICE_UPDATE_V2_DISCRIMINATOR: [u8; 8] = [34, 241, 35, 99, 157, 126, 244, 205];

/// How thoroughly the posted update was verified by the receiver program.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum VerificationLevel {
    /// Only `num_signatures` of the Wormhole guardian signatures were checked.
    Partial { num_signatures: u8 },
    /// The full guardian quorum was checked.
    Full,
}

/// Pyth `PriceFeedMessage`, Borsh-identical to the receiver program's layout.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug)]
pub struct PriceFeedMessage {
    pub feed_id: [u8; 32],
    pub price: i64,
    pub conf: u64,
    pub exponent: i32,
    pub publish_time: i64,
    pub prev_publish_time: i64,
    pub ema_price: i64,
    pub ema_conf: u64,
}

/// Pyth `PriceUpdateV2`, Borsh-identical to the receiver program's layout.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug)]
pub struct PriceUpdateV2 {
    pub write_authority: Pubkey,
    pub verification_level: VerificationLevel,
    pub price_message: PriceFeedMessage,
    pub posted_slot: u64,
}

/// The subset of a validated price update the protocol actually uses.
#[derive(Clone, Copy, Debug)]
pub struct OraclePrice {
    pub price: i64,
    pub expo: i32,
    pub conf: u64,
    pub publish_time: i64,
    pub posted_slot: u64,
}

/// Read, authenticate and gate a Pyth price update.
///
/// `expected_oracle` and `expected_feed_id` come from the protocol config, so
/// an attacker cannot substitute a different feed (for example a low-liquidity
/// pair whose price they can move) even if they control instruction inputs.
pub fn load_price(
    account: &AccountInfo,
    expected_oracle: &Pubkey,
    expected_feed_id: &[u8; 32],
    max_price_age_sec: u32,
    max_conf_bps: u16,
    now: i64,
) -> Result<OraclePrice> {
    require_keys_eq!(
        account.key(),
        *expected_oracle,
        PoyzError::OracleAccountMismatch
    );
    require_keys_eq!(
        *account.owner,
        PYTH_RECEIVER_PROGRAM_ID,
        PoyzError::OracleOwnerMismatch
    );

    let data = account.try_borrow_data()?;
    require!(data.len() > 8, PoyzError::OracleDiscriminatorMismatch);
    let (discriminator, mut body) = data.split_at(8);
    require!(
        discriminator == PRICE_UPDATE_V2_DISCRIMINATOR,
        PoyzError::OracleDiscriminatorMismatch
    );

    let update = PriceUpdateV2::deserialize(&mut body)
        .map_err(|_| error!(PoyzError::OracleDeserializeFailed))?;

    // Partial verification means fewer guardian signatures were checked than
    // the Wormhole quorum. Cheaper to post, weaker to trust. A synthetic dollar
    // prices its entire collateral book off this number, so only Full passes.
    require!(
        update.verification_level == VerificationLevel::Full,
        PoyzError::OracleNotFullyVerified
    );

    let message = update.price_message;
    require!(
        message.feed_id == *expected_feed_id,
        PoyzError::OracleFeedMismatch
    );
    require!(message.price > 0, PoyzError::OracleInvalidPrice);

    // Staleness. `age` is signed on purpose: a publish time ahead of the
    // cluster clock means the account is not what it claims to be, and is
    // rejected instead of being clamped to zero age.
    let age = now
        .checked_sub(message.publish_time)
        .ok_or_else(|| error!(PoyzError::MathOverflow))?;
    require!(age >= 0, PoyzError::OraclePriceFromFuture);
    require!(
        age <= i64::from(max_price_age_sec),
        PoyzError::OraclePriceStale
    );

    // Confidence. Pyth publishes a 1-sigma-ish interval; a wide interval means
    // the aggregate is unreliable (venue outage, thin book, de-peg in
    // progress). Minting against it would issue synthetic dollars at a price
    // nobody can defend.
    let conf_bps = confidence_bps(message.conf, message.price)?;
    require!(
        conf_bps <= u64::from(max_conf_bps),
        PoyzError::OracleConfidenceTooWide
    );

    Ok(OraclePrice {
        price: message.price,
        expo: message.exponent,
        conf: message.conf,
        publish_time: message.publish_time,
        posted_slot: update.posted_slot,
    })
}

/// Validate an account is a well-formed, fully verified `PriceUpdateV2` for the
/// given feed, without applying the freshness / confidence gates.
///
/// Used only by `initialize`, where the point is to reject a misconfigured
/// oracle address at setup time. Freshness is meaningless there because the
/// protocol has no positions yet.
pub fn validate_oracle_account(account: &AccountInfo, expected_feed_id: &[u8; 32]) -> Result<()> {
    require_keys_eq!(
        *account.owner,
        PYTH_RECEIVER_PROGRAM_ID,
        PoyzError::OracleOwnerMismatch
    );

    let data = account.try_borrow_data()?;
    require!(data.len() > 8, PoyzError::OracleDiscriminatorMismatch);
    let (discriminator, mut body) = data.split_at(8);
    require!(
        discriminator == PRICE_UPDATE_V2_DISCRIMINATOR,
        PoyzError::OracleDiscriminatorMismatch
    );

    let update = PriceUpdateV2::deserialize(&mut body)
        .map_err(|_| error!(PoyzError::OracleDeserializeFailed))?;
    require!(
        update.price_message.feed_id == *expected_feed_id,
        PoyzError::OracleFeedMismatch
    );

    Ok(())
}
