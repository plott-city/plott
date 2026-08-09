//! Every failure mode this program can produce has a named error.
//!
//! There is no `unwrap()`, `expect()` or `panic!()` anywhere in `programs/`.
//! A panic inside an SBF program aborts with a generic error code that carries
//! no information to the caller and no information to an indexer, so every
//! fallible path here goes through `require!`, `?` or an explicit `ok_or`.

use anchor_lang::prelude::*;

#[error_code]
pub enum PoyzError {
    // -- authority ---------------------------------------------------------
    #[msg("Signer is not the protocol authority.")]
    Unauthorized,
    #[msg("No authority transfer is pending.")]
    NoPendingAuthority,
    #[msg("Signer is not the pending authority.")]
    NotPendingAuthority,
    #[msg("The guardian may pause but never unpause.")]
    GuardianCannotUnpause,

    // -- lifecycle ---------------------------------------------------------
    #[msg("Minting is paused.")]
    MintPaused,
    #[msg("Redemption is paused.")]
    RedeemPaused,
    #[msg("Protocol vaults are not fully initialized yet.")]
    VaultsNotReady,
    #[msg("Vault has already been initialized.")]
    VaultAlreadyInitialized,

    // -- parameters --------------------------------------------------------
    #[msg("Basis-point parameter exceeds 10000.")]
    InvalidBps,
    #[msg("Collateral ratio must be at least 10000 bps (1.00x).")]
    InvalidCollateralRatio,
    #[msg("Delta band must be 1..=2000 bps and the exit target must sit inside it.")]
    InvalidDeltaThreshold,
    #[msg("Oracle staleness bound must be between 1 and 3600 seconds.")]
    InvalidPriceAge,
    #[msg("Mint decimals are out of the supported range.")]
    InvalidDecimals,
    #[msg("Synthetic mint authority must be the protocol config PDA.")]
    InvalidMintAuthority,
    #[msg("Synthetic mint must not carry a freeze authority.")]
    FreezeAuthoritySet,
    #[msg("All protocol mints must belong to the same token program.")]
    TokenProgramMismatch,
    #[msg("Amount must be greater than zero.")]
    ZeroAmount,
    #[msg("Requested amount exceeds the configured limit.")]
    LimitExceeded,

    // -- oracle ------------------------------------------------------------
    #[msg("Oracle account does not match the configured price feed account.")]
    OracleAccountMismatch,
    #[msg("Oracle account is not owned by the Pyth receiver program.")]
    OracleOwnerMismatch,
    #[msg("Oracle account discriminator is not PriceUpdateV2.")]
    OracleDiscriminatorMismatch,
    #[msg("Oracle account data could not be deserialized.")]
    OracleDeserializeFailed,
    #[msg("Oracle price update is not fully verified.")]
    OracleNotFullyVerified,
    #[msg("Oracle feed id does not match the configured feed id.")]
    OracleFeedMismatch,
    #[msg("Oracle price is zero or negative.")]
    OracleInvalidPrice,
    #[msg("Oracle price is older than the configured staleness bound.")]
    OraclePriceStale,
    #[msg("Oracle publish time is in the future.")]
    OraclePriceFromFuture,
    #[msg("Oracle confidence interval is wider than the configured bound.")]
    OracleConfidenceTooWide,

    // -- venue state, carry, capacity --------------------------------------
    #[msg("Venue id is not a real, enabled hedge venue.")]
    VenueNotEnabled,
    #[msg("No venue state has been reported yet.")]
    VenueStateMissing,
    #[msg("Reported venue state is older than the configured bound.")]
    VenueStateStale,
    #[msg("Reported venue state is timestamped in the future.")]
    VenueStateFromFuture,
    #[msg("Reported net carry is outside the representable range.")]
    CarryOutOfRange,
    #[msg("Net carry is below the issuance floor; minting is refused.")]
    CarryBelowFloor,
    #[msg("Outstanding supply would exceed the hedgeable venue capacity.")]
    VenueCapacityExceeded,
    #[msg("Book delta is outside the hard band; minting is refused.")]
    DeltaOutsideHardBand,

    // -- keeper ------------------------------------------------------------
    #[msg("Keeper bond is below the protocol minimum.")]
    InsufficientBond,
    #[msg("Keeper is not active.")]
    KeeperInactive,
    #[msg("Keeper account does not belong to this protocol config.")]
    KeeperMismatch,
    #[msg("Slash amount exceeds the keeper bond.")]
    SlashExceedsBond,
    #[msg("Slash reason code is not one of the enumerated faults.")]
    UnknownSlashReason,
    #[msg("Unbond cooldown since the last committed proof has not elapsed.")]
    UnbondCooldownActive,
    #[msg("Withdrawal would drop the bond below the protocol minimum without a full exit.")]
    BondBelowMinimum,

    // -- execution proof ---------------------------------------------------
    #[msg("Proof sequence does not match the protocol rebalance counter.")]
    ProofSequenceMismatch,
    #[msg("Proof slot is not strictly greater than the last committed proof slot.")]
    ProofSlotNotMonotonic,
    #[msg("Proof hash is empty.")]
    EmptyProofHash,
    #[msg("Post-rebalance delta deviation is outside the inner exit target.")]
    DeltaThresholdExceeded,
    #[msg("Reported delta deviation is outside the representable range.")]
    DeltaOutOfRange,
    #[msg("Reported collateral notional disagrees with the on-chain valuation.")]
    ProofCollateralMismatch,
    #[msg("Reported post-rebalance delta disagrees with the on-chain valuation.")]
    ProofDeltaMismatch,

    // -- mint / redeem -----------------------------------------------------
    #[msg("Request has expired.")]
    RequestExpired,
    #[msg("Request has not expired yet; only the assigned keeper may act.")]
    RequestNotExpired,
    #[msg("Settlement delay since the request has not elapsed.")]
    SettlementDelayActive,
    #[msg("Hedge fill is smaller than the required notional after slippage.")]
    HedgeFillTooSmall,
    #[msg("Hedge fill is larger than the notional being issued against.")]
    HedgeFillTooLarge,
    #[msg("Resulting synthetic amount is below the caller's minimum.")]
    SlippageExceeded,
    #[msg("Redeem amount exceeds the outstanding synthetic supply.")]
    RedeemExceedsSupply,
    #[msg("Redeem would release more collateral than the protocol holds.")]
    RedeemExceedsCollateral,
    #[msg("Synthetic supply cap would be exceeded.")]
    SupplyCapExceeded,

    // -- funding / staking / buffer ---------------------------------------
    #[msg("Nothing is staked, so funding cannot be distributed to stakers.")]
    NoStakers,
    #[msg("Staked balance is smaller than the requested amount.")]
    InsufficientStake,
    #[msg("There is nothing to claim.")]
    NothingToClaim,
    #[msg("Unstake cooldown has not elapsed.")]
    UnstakeCooldownActive,
    #[msg("There is no pending unstake to withdraw.")]
    NoPendingUnstake,
    #[msg("Insurance buffer balance is smaller than the requested amount.")]
    InsufficientBuffer,
    #[msg("Insurance buffer is locked: funding is not in a sustained negative regime.")]
    BufferLocked,
    #[msg("Withdrawal exceeds the per-call insurance buffer draw cap.")]
    BufferDrawCapExceeded,

    // -- arithmetic --------------------------------------------------------
    #[msg("Arithmetic overflow.")]
    MathOverflow,
}
