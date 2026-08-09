/**
 * Poyz localnet suite.
 *
 * Runs against a `solana-test-validator` started by `anchor test`. Ordered:
 * later blocks build on state created by earlier ones, which is deliberate --
 * the point of the happy path is that one collateral deposit survives a hedge
 * attestation, a rebalance proof, a funding cycle and a redemption without the
 * protocol's books drifting.
 */

import * as anchor from "@coral-xyz/anchor";
import { BN } from "@coral-xyz/anchor";
import { transfer, TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { SystemProgram } from "@solana/web3.js";
import { assert } from "chai";

import {
  Env,
  FEED_ID,
  ORACLE_HEALTHY,
  ORACLE_PARTIAL,
  ORACLE_SHIFTED,
  ORACLE_WIDE_CONF,
  MAX_REPORTABLE_CAPACITY,
  MEASURED_CARRY_1Y,
  MEASURED_CARRY_30D,
  PARAMS,
  PROOF_HASH,
  REFERENCE_MIN_NET_CARRY_BPS,
  SLASH_REASON_FALSE_PROOF,
  VENUE_CAPACITY,
  VENUE_CARRY_BPS,
  VENUE_FLAGS_VELOCITY_ONLY,
  VENUE_JUPITER_PERPS,
  VENUE_NONE,
  VENUE_SIMULATED,
  VENUE_VELOCITY,
  venueContract,
  expectError,
  expectFailure,
  setupEnv,
  sleep,
  tokenBalance,
} from "./helpers/env";

/** `UpdateParams` with every field `None` unless overridden. */
type UpdateOverrides = Partial<{
  maxPriceAgeSec: number;
  maxConfBps: number;
  collateralRatioBps: number;
  mintFeeBps: number;
  redeemFeeBps: number;
  deltaBandBps: number;
  deltaExitBps: number;
  deltaHardBps: number;
  maxHedgeSlippageBps: number;
  bufferShareBps: number;
  bufferMaxDrawBps: number;
  minKeeperBond: BN;
  maxSyntheticSupply: BN;
  requestTtlSec: number;
  minSettlementDelaySec: number;
  unbondCooldownSec: number;
  bufferUnlockDelaySec: number;
  unstakeCooldownSec: number;
  maxSupplyVsCapacityBps: number;
  maxVenueStateAgeSec: number;
  minNetCarryBps: number;
  maxReportableCapacityNotional: BN;
  venueFlags: number;
}>;

function updateParams(overrides: UpdateOverrides) {
  return {
    maxPriceAgeSec: overrides.maxPriceAgeSec ?? null,
    maxConfBps: overrides.maxConfBps ?? null,
    collateralRatioBps: overrides.collateralRatioBps ?? null,
    mintFeeBps: overrides.mintFeeBps ?? null,
    redeemFeeBps: overrides.redeemFeeBps ?? null,
    deltaBandBps: overrides.deltaBandBps ?? null,
    deltaExitBps: overrides.deltaExitBps ?? null,
    deltaHardBps: overrides.deltaHardBps ?? null,
    maxHedgeSlippageBps: overrides.maxHedgeSlippageBps ?? null,
    bufferShareBps: overrides.bufferShareBps ?? null,
    bufferMaxDrawBps: overrides.bufferMaxDrawBps ?? null,
    minKeeperBond: overrides.minKeeperBond ?? null,
    maxSyntheticSupply: overrides.maxSyntheticSupply ?? null,
    requestTtlSec: overrides.requestTtlSec ?? null,
    minSettlementDelaySec: overrides.minSettlementDelaySec ?? null,
    unbondCooldownSec: overrides.unbondCooldownSec ?? null,
    bufferUnlockDelaySec: overrides.bufferUnlockDelaySec ?? null,
    unstakeCooldownSec: overrides.unstakeCooldownSec ?? null,
    maxSupplyVsCapacityBps: overrides.maxSupplyVsCapacityBps ?? null,
    maxVenueStateAgeSec: overrides.maxVenueStateAgeSec ?? null,
    minNetCarryBps: overrides.minNetCarryBps ?? null,
    maxReportableCapacityNotional: overrides.maxReportableCapacityNotional ?? null,
    venueFlags: overrides.venueFlags ?? null,
  };
}

// SOL at 152.34 with Pyth exponent -8; 9-decimal collateral, 6-decimal
// synthetic. Every expected value below is derived from these by hand so a
// silent change in the rounding direction shows up as a failing assertion.
const DEPOSIT = new BN(10_000_000_000); // 10 SOL
const EXPECTED_NOTIONAL = 1_523_400_000; // 1523.40 pUSD
const EXPECTED_MINT_FEE = 1_523_400; // ceil(0.10 %)
const EXPECTED_MINTED = EXPECTED_NOTIONAL - EXPECTED_MINT_FEE;

const STAKE_AMOUNT = new BN(500_000_000); // 500 pUSD
const FUNDING_AMOUNT = new BN(100_000_000); // 100 pUSD
const EXPECTED_TO_BUFFER = 10_000_000; // 10 % share, rounded toward the buffer
const EXPECTED_TO_STAKERS = 90_000_000;

const REDEEM_AMOUNT = new BN(300_000_000); // 300 pUSD
const EXPECTED_REDEEM_FEE = 300_000; // ceil(0.10 %)
// floor((300_000_000 - fee) * 1e11 / 15_234_000_000): rounded down, so the
// redeemer never gets more collateral than the burned synthetic was worth.
const EXPECTED_COLLATERAL_OUT = 1_967_309_964;
// What a fee-free redemption of the same 300 pUSD would have released. The
// difference stays in the vault as overcollateralization.
const COLLATERAL_BEFORE_FEE = 1_969_279_243;

describe("poyz", () => {
  let env: Env;

  before(async function () {
    this.timeout(300_000);
    env = await setupEnv();
  });

  // -------------------------------------------------------------------------
  describe("protocol setup", () => {
    it("stores the configuration and comes up with every vault ready", async () => {
      const config = await env.program.account.config.fetch(env.config);

      assert.equal(config.authority.toBase58(), env.payer.publicKey.toBase58());
      assert.equal(config.syntheticMint.toBase58(), env.syntheticMint.toBase58());
      assert.equal(config.oracle.toBase58(), ORACLE_HEALTHY.toBase58());
      assert.equal(config.tokenProgram.toBase58(), TOKEN_PROGRAM_ID.toBase58());
      assert.equal(config.collateralDecimals, 9);
      assert.equal(config.syntheticDecimals, 6);
      assert.equal(config.vaultFlags, 0b1111, "all four vault groups initialized");
      assert.isFalse(config.mintPaused);
      assert.isFalse(config.redeemPaused);
      assert.equal(config.guardian.toBase58(), env.guardian.publicKey.toBase58());
      assert.equal(
        config.venueFlags,
        VENUE_FLAGS_VELOCITY_ONLY,
        "Velocity enabled, the rest are not"
      );
      assert.equal(config.lastVenueId, VENUE_VELOCITY);
      assert.equal(config.lastNetCarryBps, VENUE_CARRY_BPS);
      assert.equal(
        config.venueCapacityNotional.toString(),
        VENUE_CAPACITY.toString()
      );
      assert.notEqual(config.venueStateAt.toString(), "0");
      assert.equal(config.totalSynthetic.toString(), "0");
      assert.deepEqual(Array.from(config.feedId), FEED_ID);
    });

    it("publishes a venue contract that matches the on-chain ids", () => {
      // idl/venues.json is what every off-chain package turns a router's venue
      // *string* into. If it drifts from the ids the program enforces, proofs
      // get attributed to the wrong venue and nothing fails to compile.
      assert.equal(venueContract.venues.velocity, VENUE_VELOCITY);
      assert.equal(venueContract.venues["jupiter-perps"], VENUE_JUPITER_PERPS);
      assert.equal(venueContract.venues.none, VENUE_NONE);
      assert.equal(venueContract.venues.simulated, VENUE_SIMULATED);
      assert.equal(venueContract.idBase, 1);
      assert.equal(venueContract.unsetId, 0);
      assert.equal(venueContract.defaultVenueFlags, VENUE_FLAGS_VELOCITY_ONLY);
      // The rename alias, not a second venue: hedge-router emits `velocity`
      // where older code emitted `drift`, and both have to reach slot 1.
      assert.equal(venueContract.aliases.drift, "velocity");
      assert.equal(
        venueContract.venues[venueContract.aliases.drift],
        VENUE_VELOCITY
      );
      // Wound-down venues resolve to nothing at all rather than to an id.
      assert.isUndefined(venueContract.venues.zeta);
      assert.property(venueContract.retired, "zeta");
      assert.property(venueContract.retired, "mango-v4");
    });

    it("rejects set_params from an account that is not the authority", async () => {
      await expectError(
        env.program.methods
          .setParams(updateParams({ mintFeeBps: 500 }))
          .accountsPartial({ authority: env.outsider.publicKey, config: env.config })
          .signers([env.outsider])
          .rpc(),
        "Unauthorized"
      );
    });

    it("refuses a fee above the hard cap even from the authority", async () => {
      await expectError(
        env.program.methods
          .setParams(updateParams({ mintFeeBps: 501 }))
          .accountsPartial({ authority: env.payer.publicKey, config: env.config })
          .rpc(),
        "InvalidBps"
      );
    });

    it("refuses a collateral ratio below 1.00x", async () => {
      await expectError(
        env.program.methods
          .setParams(updateParams({ collateralRatioBps: 9_999 }))
          .accountsPartial({ authority: env.payer.publicKey, config: env.config })
          .rpc(),
        "InvalidCollateralRatio"
      );
    });
  });

  // -------------------------------------------------------------------------
  describe("keeper bonding", () => {
    it("registers a keeper and moves the bond into the bond vault", async () => {
      await env.program.methods
        .keeperRegister(new BN(2_000_000_000))
        .accountsPartial({
          keeper: env.keeper.publicKey,
          config: env.config,
          keeperAccount: env.keeperAccount,
          bondMint: env.bondMint,
          keeperBondSource: env.keeperBondAta,
          bondVault: env.bondVault,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([env.keeper])
        .rpc();

      const keeper = await env.program.account.keeper.fetch(env.keeperAccount);
      assert.equal(keeper.bonded.toString(), "2000000000");
      assert.isTrue(keeper.active);
      assert.equal(await tokenBalance(env.provider, env.bondVault), 2_000_000_000n);

      const config = await env.program.account.config.fetch(env.config);
      assert.equal(config.keeperCount, 1);
      assert.equal(config.bondedTotal.toString(), "2000000000");
    });

    it("rejects a registration below the minimum bond", async () => {
      await expectError(
        env.program.methods
          .keeperRegister(new BN(999_999_999))
          .accountsPartial({
            keeper: env.keeper2.publicKey,
            config: env.config,
            keeperAccount: env.keeper2Account,
            bondMint: env.bondMint,
            keeperBondSource: env.keeper2BondAta,
            bondVault: env.bondVault,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([env.keeper2])
          .rpc(),
        "InsufficientBond"
      );
    });

    it("registers a second keeper at exactly the minimum", async () => {
      await env.program.methods
        .keeperRegister(PARAMS.minKeeperBond)
        .accountsPartial({
          keeper: env.keeper2.publicKey,
          config: env.config,
          keeperAccount: env.keeper2Account,
          bondMint: env.bondMint,
          keeperBondSource: env.keeper2BondAta,
          bondVault: env.bondVault,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([env.keeper2])
        .rpc();

      const config = await env.program.account.config.fetch(env.config);
      assert.equal(config.keeperCount, 2);
    });
  });

  // -------------------------------------------------------------------------
  describe("happy path: mint -> proof -> funding -> redeem", () => {
    const NONCE = 1;

    it("mint_request escrows collateral and quotes the notional", async () => {
      const request = env.mintRequestPda(env.user.publicKey, NONCE);

      await env.program.methods
        .mintRequest(new BN(NONCE), DEPOSIT, new BN(0))
        .accountsPartial({
          user: env.user.publicKey,
          config: env.config,
          request,
          collateralMint: env.collateralMint,
          userCollateral: env.userCollateral,
          collateralVault: env.collateralVault,
          oracle: ORACLE_HEALTHY,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([env.user])
        .rpc();

      const stored = await env.program.account.mintRequest.fetch(request);
      assert.equal(stored.collateralAmount.toString(), DEPOSIT.toString());
      assert.equal(stored.quotedNotional.toString(), String(EXPECTED_NOTIONAL));

      const config = await env.program.account.config.fetch(env.config);
      // Escrowed collateral is pending, not backing: nothing is hedged yet.
      assert.equal(config.pendingCollateral.toString(), DEPOSIT.toString());
      assert.equal(config.totalCollateral.toString(), "0");
      assert.equal(config.totalSynthetic.toString(), "0");
      assert.equal(
        await tokenBalance(env.provider, env.collateralVault),
        BigInt(DEPOSIT.toString())
      );
    });

    it("mint_confirm rejects a hedge fill below the slippage band", async () => {
      const request = env.mintRequestPda(env.user.publicKey, NONCE);
      await expectError(
        env.program.methods
          .mintConfirm(new BN(NONCE), PROOF_HASH(1), VENUE_VELOCITY, new BN(1_000_000_000))
          .accountsPartial({
            keeper: env.keeper.publicKey,
            config: env.config,
            keeperAccount: env.keeperAccount,
            request,
            user: env.user.publicKey,
            syntheticMint: env.syntheticMint,
            userSynthetic: env.userSynthetic,
            oracle: ORACLE_HEALTHY,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([env.keeper])
          .rpc(),
        "HedgeFillTooSmall"
      );
    });

    it("mint_confirm issues synthetic against an attested hedge", async () => {
      const request = env.mintRequestPda(env.user.publicKey, NONCE);

      await env.program.methods
        .mintConfirm(new BN(NONCE), PROOF_HASH(1), VENUE_VELOCITY, new BN(EXPECTED_NOTIONAL))
        .accountsPartial({
          keeper: env.keeper.publicKey,
          config: env.config,
          keeperAccount: env.keeperAccount,
          request,
          user: env.user.publicKey,
          syntheticMint: env.syntheticMint,
          userSynthetic: env.userSynthetic,
          oracle: ORACLE_HEALTHY,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([env.keeper])
        .rpc();

      assert.equal(
        await tokenBalance(env.provider, env.userSynthetic),
        BigInt(EXPECTED_MINTED),
        "user receives notional minus the ceil-rounded fee"
      );

      const config = await env.program.account.config.fetch(env.config);
      assert.equal(config.pendingCollateral.toString(), "0");
      assert.equal(config.totalCollateral.toString(), DEPOSIT.toString());
      assert.equal(config.totalSynthetic.toString(), String(EXPECTED_MINTED));
      assert.equal(config.hedgedNotional.toString(), String(EXPECTED_NOTIONAL));

      // The request account is closed and its rent refunded to the user.
      assert.isNull(await env.provider.connection.getAccountInfo(request));
    });

    it("commit_rebalance_proof records a sequenced, oracle-stamped proof", async () => {
      const proof = env.proofPda(0);

      await env.program.methods
        .commitRebalanceProof(
          new BN(0),
          PROOF_HASH(7),
          VENUE_VELOCITY,
          120, // delta before: 1.20 %, outside the 1.00 % trigger band
          -8, // delta after, as the keeper computed it
          new BN(EXPECTED_NOTIONAL), // hedged notional -- the attested leg
          new BN(EXPECTED_NOTIONAL) // collateral notional -- the program re-derives this
        )
        .accountsPartial({
          keeper: env.keeper.publicKey,
          config: env.config,
          keeperAccount: env.keeperAccount,
          proof,
          oracle: ORACLE_HEALTHY,
          systemProgram: SystemProgram.programId,
        })
        .signers([env.keeper])
        .rpc();

      const stored = await env.program.account.rebalanceProof.fetch(proof);
      assert.equal(stored.sequence.toString(), "0");
      assert.equal(stored.deltaBpsBefore, 120, "the pre-state is keeper-reported");
      assert.equal(stored.oraclePrice.toString(), "15234000000");
      assert.equal(stored.oracleExpo, -8);
      assert.isAbove(Number(stored.slot), 0);
      assert.deepEqual(Array.from(stored.venuesHash), PROOF_HASH(7));
      assert.deepEqual(Array.from(stored.prevHash), new Array(32).fill(0));
      assert.equal(stored.venueId, VENUE_VELOCITY);
      // The record holds the program's own valuation, not the keeper's claim:
      // the keeper said -8 bps, the chain derived 0 from its own books.
      assert.equal(stored.deltaBpsAfter, 0);
      assert.equal(
        stored.collateralNotional.toString(),
        String(EXPECTED_NOTIONAL),
        "collateral notional is recomputed on-chain"
      );

      const config = await env.program.account.config.fetch(env.config);
      assert.equal(config.rebalanceCount.toString(), "1");
      // The chain head is computed by the program, not supplied by the keeper.
      assert.deepEqual(
        Array.from(config.lastProofHash),
        Array.from(stored.thisHash),
        "config carries the chain head"
      );
      assert.notDeepEqual(
        Array.from(stored.thisHash),
        Array.from(stored.venuesHash),
        "the chain head is not just the keeper's payload digest"
      );
    });

    it("rejects a proof whose sequence does not follow the chain", async () => {
      await expectError(
        env.program.methods
          .commitRebalanceProof(
            new BN(5), // rebalance_count is 1
            PROOF_HASH(9),
            VENUE_VELOCITY,
            10,
            0,
            new BN(EXPECTED_NOTIONAL),
            new BN(EXPECTED_NOTIONAL)
          )
          .accountsPartial({
            keeper: env.keeper.publicKey,
            config: env.config,
            keeperAccount: env.keeperAccount,
            proof: env.proofPda(5),
            oracle: ORACLE_HEALTHY,
            systemProgram: SystemProgram.programId,
          })
          .signers([env.keeper])
          .rpc(),
        "ProofSequenceMismatch"
      );
    });

    it("rejects a replay of an already committed sequence", async () => {
      // The proof PDA for sequence 0 exists, so the replay never reaches the
      // handler: account creation fails first. That is the intended second
      // line of defence, and it produces a system-program error rather than an
      // Anchor error code.
      await expectFailure(
        env.program.methods
          .commitRebalanceProof(
            new BN(0),
            PROOF_HASH(9),
            VENUE_VELOCITY,
            10,
            0,
            new BN(EXPECTED_NOTIONAL),
            new BN(EXPECTED_NOTIONAL)
          )
          .accountsPartial({
            keeper: env.keeper.publicKey,
            config: env.config,
            keeperAccount: env.keeperAccount,
            proof: env.proofPda(0),
            oracle: ORACLE_HEALTHY,
            systemProgram: SystemProgram.programId,
          })
          .signers([env.keeper])
          .rpc(),
        /already in use/
      );
    });

    const proofAccounts = (sequence: number) => ({
      keeper: env.keeper.publicKey,
      config: env.config,
      keeperAccount: env.keeperAccount,
      proof: env.proofPda(sequence),
      oracle: ORACLE_HEALTHY,
      systemProgram: SystemProgram.programId,
    });

    const proofWithVenue = (venueId: number, hash: number[]) =>
      env.program.methods
        .commitRebalanceProof(
          new BN(1),
          hash,
          venueId,
          0,
          0,
          new BN(EXPECTED_NOTIONAL),
          new BN(EXPECTED_NOTIONAL)
        )
        .accountsPartial(proofAccounts(1))
        .signers([env.keeper])
        .rpc();

    it("rejects a proof whose venue id was never set", async () => {
      // The reason the mapping is 1-based. A caller that forgot to populate
      // venue_id sends a zero byte; if 0 meant Velocity this proof would be
      // silently attributed to the primary venue instead of rejected.
      await expectError(proofWithVenue(VENUE_NONE, PROOF_HASH(8)), "VenueNotEnabled");
    });

    it("rejects a proof for a simulated fill", async () => {
      await expectError(
        proofWithVenue(VENUE_SIMULATED, PROOF_HASH(9)),
        "VenueNotEnabled"
      );
    });

    it("rejects a proof for a real venue that is not enabled", async () => {
      await expectError(
        proofWithVenue(VENUE_JUPITER_PERPS, PROOF_HASH(10)),
        "VenueNotEnabled"
      );
    });

    it("rejects a proof whose collateral valuation disagrees with the chain", async function () {
      this.timeout(30_000);
      // The collateral check sits after the slot-monotonicity guard, so a fresh
      // slot is needed to reach it.
      await sleep(1_500);
      await expectError(
        env.program.methods
          .commitRebalanceProof(
            new BN(1),
            PROOF_HASH(12),
            VENUE_VELOCITY,
            0,
            0,
            new BN(EXPECTED_NOTIONAL),
            new BN(1_000_000) // nowhere near the on-chain book value
          )
          .accountsPartial(proofAccounts(1))
          .signers([env.keeper])
          .rpc(),
        "ProofCollateralMismatch"
      );
    });

    it("rejects a proof that leaves the book outside the exit target", async function () {
      this.timeout(30_000);
      // A proof in the same slot as the previous one trips the slot
      // monotonicity guard before the delta check, so wait for a new slot to
      // exercise the check under test.
      await sleep(1_500);
      await expectError(
        env.program.methods
          .commitRebalanceProof(
            new BN(1),
            PROOF_HASH(11),
            VENUE_VELOCITY,
            300,
            3_435, // even reported honestly, the book is 34 % under-hedged
            new BN(1_000_000_000), // attested short far below the collateral leg
            new BN(EXPECTED_NOTIONAL)
          )
          .accountsPartial(proofAccounts(1))
          .signers([env.keeper])
          .rpc(),
        "DeltaThresholdExceeded"
      );
    });

    it("rejects a proof whose reported delta disagrees with the chain", async function () {
      this.timeout(30_000);
      await sleep(1_500);
      // The book is perfectly hedged, so the chain derives 0. A keeper claiming
      // 200 bps is running on a book it has mis-modelled.
      await expectError(
        env.program.methods
          .commitRebalanceProof(
            new BN(1),
            PROOF_HASH(13),
            VENUE_VELOCITY,
            0,
            200,
            new BN(EXPECTED_NOTIONAL),
            new BN(EXPECTED_NOTIONAL)
          )
          .accountsPartial(proofAccounts(1))
          .signers([env.keeper])
          .rpc(),
        "ProofDeltaMismatch"
      );
    });

    it("stakes synthetic and distributes a funding settlement pro rata", async () => {
      // The treasury needs synthetic dollars to settle with. It acquires them
      // the same way anyone else does; here the user simply hands some over.
      await transfer(
        env.provider.connection,
        env.payer,
        env.userSynthetic,
        env.authoritySynthetic,
        env.user,
        BigInt(FUNDING_AMOUNT.toString())
      );

      const position = env.stakePositionPda(env.user.publicKey);
      await env.program.methods
        .stake(STAKE_AMOUNT)
        .accountsPartial({
          owner: env.user.publicKey,
          config: env.config,
          position,
          syntheticMint: env.syntheticMint,
          ownerSynthetic: env.userSynthetic,
          stakeVault: env.stakeVault,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([env.user])
        .rpc();

      await env.program.methods
        .settleFunding(FUNDING_AMOUNT)
        .accountsPartial({
          authority: env.payer.publicKey,
          config: env.config,
          syntheticMint: env.syntheticMint,
          authoritySynthetic: env.authoritySynthetic,
          fundingVault: env.fundingVault,
          bufferVault: env.bufferVault,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .rpc();

      const config = await env.program.account.config.fetch(env.config);
      assert.equal(config.totalStaked.toString(), STAKE_AMOUNT.toString());
      assert.equal(config.bufferBalance.toString(), String(EXPECTED_TO_BUFFER));
      assert.equal(config.stakerFundingBalance.toString(), String(EXPECTED_TO_STAKERS));
      assert.equal(config.negativeFundingSince.toString(), "0", "positive funding");
      assert.equal(
        await tokenBalance(env.provider, env.fundingVault),
        BigInt(EXPECTED_TO_STAKERS)
      );
      assert.equal(
        await tokenBalance(env.provider, env.bufferVault),
        BigInt(EXPECTED_TO_BUFFER)
      );
    });

    it("claims the whole settlement for the only staker", async () => {
      const before = await tokenBalance(env.provider, env.userSynthetic);

      await env.program.methods
        .claimFunding()
        .accountsPartial({
          owner: env.user.publicKey,
          config: env.config,
          position: env.stakePositionPda(env.user.publicKey),
          syntheticMint: env.syntheticMint,
          fundingVault: env.fundingVault,
          ownerSynthetic: env.userSynthetic,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([env.user])
        .rpc();

      const after = await tokenBalance(env.provider, env.userSynthetic);
      assert.equal(after - before, BigInt(EXPECTED_TO_STAKERS));

      const config = await env.program.account.config.fetch(env.config);
      assert.equal(config.stakerFundingBalance.toString(), "0");

      await expectError(
        env.program.methods
          .claimFunding()
          .accountsPartial({
            owner: env.user.publicKey,
            config: env.config,
            position: env.stakePositionPda(env.user.publicKey),
            syntheticMint: env.syntheticMint,
            fundingVault: env.fundingVault,
            ownerSynthetic: env.userSynthetic,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([env.user])
          .rpc(),
        "NothingToClaim"
      );
    });

    it("redeem_request escrows the synthetic", async () => {
      const request = env.redeemRequestPda(env.user.publicKey, 1);

      await env.program.methods
        .redeemRequest(new BN(1), REDEEM_AMOUNT, new BN(0))
        .accountsPartial({
          user: env.user.publicKey,
          config: env.config,
          request,
          syntheticMint: env.syntheticMint,
          userSynthetic: env.userSynthetic,
          redeemEscrow: env.redeemEscrow,
          oracle: ORACLE_HEALTHY,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([env.user])
        .rpc();

      const stored = await env.program.account.redeemRequest.fetch(request);
      assert.equal(stored.syntheticAmount.toString(), REDEEM_AMOUNT.toString());
      assert.equal(stored.quotedCollateral.toString(), String(EXPECTED_COLLATERAL_OUT));
      assert.equal(
        await tokenBalance(env.provider, env.redeemEscrow),
        BigInt(REDEEM_AMOUNT.toString())
      );
    });

    it("rejects a confirm before the settlement delay has elapsed", async () => {
      await expectError(
        env.program.methods
          .redeemConfirm(new BN(1), PROOF_HASH(21), VENUE_VELOCITY, REDEEM_AMOUNT)
          .accountsPartial({
            keeper: env.keeper.publicKey,
            config: env.config,
            keeperAccount: env.keeperAccount,
            request: env.redeemRequestPda(env.user.publicKey, 1),
            user: env.user.publicKey,
            syntheticMint: env.syntheticMint,
            redeemEscrow: env.redeemEscrow,
            collateralMint: env.collateralMint,
            collateralVault: env.collateralVault,
            userCollateral: env.userCollateral,
            oracle: ORACLE_HEALTHY,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([env.keeper])
          .rpc(),
        "SettlementDelayActive"
      );
    });

    it("redeem_confirm burns the synthetic and releases collateral", async function () {
      this.timeout(60_000);
      await sleep(2_500); // clear min_settlement_delay_sec

      const collateralBefore = await tokenBalance(env.provider, env.userCollateral);
      const supplyBefore = (await env.program.account.config.fetch(env.config))
        .totalSynthetic;

      await env.program.methods
        .redeemConfirm(new BN(1), PROOF_HASH(21), VENUE_VELOCITY, REDEEM_AMOUNT)
        .accountsPartial({
          keeper: env.keeper.publicKey,
          config: env.config,
          keeperAccount: env.keeperAccount,
          request: env.redeemRequestPda(env.user.publicKey, 1),
          user: env.user.publicKey,
          syntheticMint: env.syntheticMint,
          redeemEscrow: env.redeemEscrow,
          collateralMint: env.collateralMint,
          collateralVault: env.collateralVault,
          userCollateral: env.userCollateral,
          oracle: ORACLE_HEALTHY,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([env.keeper])
        .rpc();

      const collateralAfter = await tokenBalance(env.provider, env.userCollateral);
      assert.equal(
        collateralAfter - collateralBefore,
        BigInt(EXPECTED_COLLATERAL_OUT),
        "collateral released rounds down, and the redeem fee stays in the vault"
      );
      assert.equal(await tokenBalance(env.provider, env.redeemEscrow), 0n);

      const config = await env.program.account.config.fetch(env.config);
      assert.equal(
        config.totalSynthetic.toString(),
        supplyBefore.sub(REDEEM_AMOUNT).toString()
      );
      assert.equal(config.pendingRedeemSynthetic.toString(), "0");

      // The fee is retained as overcollateralization: the vault keeps the
      // collateral that a fee-free redemption would have released.
      assert.equal(
        COLLATERAL_BEFORE_FEE - EXPECTED_COLLATERAL_OUT,
        1_969_279,
        "the redeem fee stays in the collateral vault, backing the remaining supply"
      );
      assert.isBelow(EXPECTED_COLLATERAL_OUT, COLLATERAL_BEFORE_FEE);
      assert.equal(EXPECTED_REDEEM_FEE, 300_000);
    });
  });

  // -------------------------------------------------------------------------
  describe("oracle gates", () => {
    const probeMint = (nonce: number, oracle = ORACLE_HEALTHY) =>
      env.program.methods
        .mintRequest(new BN(nonce), new BN(1_000_000_000), new BN(0))
        .accountsPartial({
          user: env.user.publicKey,
          config: env.config,
          request: env.mintRequestPda(env.user.publicKey, nonce),
          collateralMint: env.collateralMint,
          userCollateral: env.userCollateral,
          collateralVault: env.collateralVault,
          oracle,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([env.user])
        .rpc();

    const setOracle = (oracle: anchor.web3.PublicKey) =>
      env.program.methods
        .setOracle(FEED_ID)
        .accountsPartial({ authority: env.payer.publicKey, config: env.config, oracle })
        .rpc();

    it("rejects an oracle account that is not the configured one", async () => {
      await expectError(probeMint(20, ORACLE_WIDE_CONF), "OracleAccountMismatch");
    });

    it("rejects a price older than the staleness bound", async () => {
      await env.program.methods
        .setParams(updateParams({ maxPriceAgeSec: 1 }))
        .accountsPartial({ authority: env.payer.publicKey, config: env.config })
        .rpc();

      await expectError(probeMint(21), "OraclePriceStale");

      await env.program.methods
        .setParams(updateParams({ maxPriceAgeSec: PARAMS.maxPriceAgeSec }))
        .accountsPartial({ authority: env.payer.publicKey, config: env.config })
        .rpc();
    });

    it("rejects a confidence interval wider than the bound", async () => {
      await setOracle(ORACLE_WIDE_CONF);
      await expectError(probeMint(22, ORACLE_WIDE_CONF), "OracleConfidenceTooWide");
      await setOracle(ORACLE_HEALTHY);
    });

    it("rejects a partially verified price update", async () => {
      await setOracle(ORACLE_PARTIAL);
      await expectError(probeMint(23, ORACLE_PARTIAL), "OracleNotFullyVerified");
      await setOracle(ORACLE_HEALTHY);
    });

    it("still mints after the oracle is restored", async () => {
      await probeMint(24);
      const stored = await env.program.account.mintRequest.fetch(
        env.mintRequestPda(env.user.publicKey, 24)
      );
      assert.equal(stored.quotedNotional.toString(), "152340000");

      // Clean up: confirm it so the escrow does not linger.
      await env.program.methods
        .mintConfirm(new BN(24), PROOF_HASH(31), VENUE_VELOCITY, new BN(152_340_000))
        .accountsPartial({
          keeper: env.keeper.publicKey,
          config: env.config,
          keeperAccount: env.keeperAccount,
          request: env.mintRequestPda(env.user.publicKey, 24),
          user: env.user.publicKey,
          syntheticMint: env.syntheticMint,
          userSynthetic: env.userSynthetic,
          oracle: ORACLE_HEALTHY,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([env.keeper])
        .rpc();
    });
  });

  // -------------------------------------------------------------------------
  describe("authorization and bonding gates", () => {
    it("rejects keeper_slash from an account that is not the authority", async () => {
      await expectError(
        env.program.methods
          .keeperSlash(new BN(1_000_000), SLASH_REASON_FALSE_PROOF, PROOF_HASH(41))
          .accountsPartial({
            authority: env.outsider.publicKey,
            config: env.config,
            keeperAccount: env.keeper2Account,
            bondMint: env.bondMint,
            bondVault: env.bondVault,
            bufferBondVault: env.bufferBondVault,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([env.outsider])
          .rpc(),
        "Unauthorized"
      );
    });

    it("rejects a slash with an empty evidence hash", async () => {
      await expectError(
        env.program.methods
          .keeperSlash(new BN(1_000_000), SLASH_REASON_FALSE_PROOF, new Array(32).fill(0))
          .accountsPartial({
            authority: env.payer.publicKey,
            config: env.config,
            keeperAccount: env.keeper2Account,
            bondMint: env.bondMint,
            bondVault: env.bondVault,
            bufferBondVault: env.bufferBondVault,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .rpc(),
        "EmptyProofHash"
      );
    });

    it("slashes a keeper below the minimum and deactivates it", async () => {
      await env.program.methods
        .keeperSlash(new BN(500_000_000), SLASH_REASON_FALSE_PROOF, PROOF_HASH(43))
        .accountsPartial({
          authority: env.payer.publicKey,
          config: env.config,
          keeperAccount: env.keeper2Account,
          bondMint: env.bondMint,
          bondVault: env.bondVault,
          bufferBondVault: env.bufferBondVault,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .rpc();

      const keeper2 = await env.program.account.keeper.fetch(env.keeper2Account);
      assert.equal(keeper2.bonded.toString(), "500000000");
      assert.equal(keeper2.slashed.toString(), "500000000");
      assert.isFalse(keeper2.active, "bond fell below the minimum");

      assert.equal(await tokenBalance(env.provider, env.bufferBondVault), 500_000_000n);

      const config = await env.program.account.config.fetch(env.config);
      assert.equal(config.keeperCount, 1);
      assert.equal(config.slashedTotal.toString(), "500000000");
    });

    it("rejects a proof commit from the under-bonded keeper", async () => {
      await expectError(
        env.program.methods
          .commitRebalanceProof(
            new BN(1),
            PROOF_HASH(45),
            VENUE_VELOCITY,
            80,
            5,
            new BN(EXPECTED_NOTIONAL),
            new BN(EXPECTED_NOTIONAL)
          )
          .accountsPartial({
            keeper: env.keeper2.publicKey,
            config: env.config,
            keeperAccount: env.keeper2Account,
            proof: env.proofPda(1),
            oracle: ORACLE_HEALTHY,
            systemProgram: SystemProgram.programId,
          })
          .signers([env.keeper2])
          .rpc(),
        "KeeperInactive"
      );
    });

    it("rejects a mint confirm from the under-bonded keeper", async () => {
      const nonce = 30;
      await env.program.methods
        .mintRequest(new BN(nonce), new BN(1_000_000_000), new BN(0))
        .accountsPartial({
          user: env.user.publicKey,
          config: env.config,
          request: env.mintRequestPda(env.user.publicKey, nonce),
          collateralMint: env.collateralMint,
          userCollateral: env.userCollateral,
          collateralVault: env.collateralVault,
          oracle: ORACLE_HEALTHY,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([env.user])
        .rpc();

      await expectError(
        env.program.methods
          .mintConfirm(new BN(nonce), PROOF_HASH(47), VENUE_VELOCITY, new BN(152_340_000))
          .accountsPartial({
            keeper: env.keeper2.publicKey,
            config: env.config,
            keeperAccount: env.keeper2Account,
            request: env.mintRequestPda(env.user.publicKey, nonce),
            user: env.user.publicKey,
            syntheticMint: env.syntheticMint,
            userSynthetic: env.userSynthetic,
            oracle: ORACLE_HEALTHY,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([env.keeper2])
          .rpc(),
        "KeeperInactive"
      );

      // Re-bonding above the minimum restores the keeper, and it can then act.
      await env.program.methods
        .keeperBond(new BN(1_000_000_000))
        .accountsPartial({
          keeper: env.keeper2.publicKey,
          config: env.config,
          keeperAccount: env.keeper2Account,
          bondMint: env.bondMint,
          keeperBondSource: env.keeper2BondAta,
          bondVault: env.bondVault,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([env.keeper2])
        .rpc();

      const keeper2 = await env.program.account.keeper.fetch(env.keeper2Account);
      assert.isTrue(keeper2.active);

      await env.program.methods
        .mintConfirm(new BN(nonce), PROOF_HASH(47), VENUE_VELOCITY, new BN(152_340_000))
        .accountsPartial({
          keeper: env.keeper2.publicKey,
          config: env.config,
          keeperAccount: env.keeper2Account,
          request: env.mintRequestPda(env.user.publicKey, nonce),
          user: env.user.publicKey,
          syntheticMint: env.syntheticMint,
          userSynthetic: env.userSynthetic,
          oracle: ORACLE_HEALTHY,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([env.keeper2])
        .rpc();
    });

    it("rejects a slash whose reason is not an enumerated fault", async () => {
      await expectError(
        env.program.methods
          .keeperSlash(new BN(1_000_000), 9, PROOF_HASH(44))
          .accountsPartial({
            authority: env.payer.publicKey,
            config: env.config,
            keeperAccount: env.keeper2Account,
            bondMint: env.bondMint,
            bondVault: env.bondVault,
            bufferBondVault: env.bufferBondVault,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .rpc(),
        "UnknownSlashReason"
      );
    });

    it("rejects an authority handover accepted by the wrong signer", async () => {
      await env.program.methods
        .transferAuthority(env.outsider.publicKey)
        .accountsPartial({ authority: env.payer.publicKey, config: env.config })
        .rpc();

      await expectError(
        env.program.methods
          .acceptAuthority()
          .accountsPartial({
            pendingAuthority: env.keeper.publicKey,
            config: env.config,
          })
          .signers([env.keeper])
          .rpc(),
        "NotPendingAuthority"
      );

      // Withdraw the proposal so the rest of the suite keeps its authority.
      await env.program.methods
        .transferAuthority(anchor.web3.PublicKey.default)
        .accountsPartial({ authority: env.payer.publicKey, config: env.config })
        .rpc();
    });
  });

  // -------------------------------------------------------------------------
  describe("circuit breakers", () => {
    const setPaused = (
      signer: anchor.web3.Keypair,
      mintPaused: boolean,
      redeemPaused: boolean
    ) =>
      env.program.methods
        .setPaused(mintPaused, redeemPaused)
        .accountsPartial({ signer: signer.publicKey, config: env.config })
        .signers(signer === env.payer ? [] : [signer])
        .rpc();

    it("rejects a pause from an account that is neither authority nor guardian", async () => {
      await expectError(setPaused(env.outsider, true, true), "Unauthorized");
    });

    it("lets the guardian halt minting while redemption stays open", async () => {
      await setPaused(env.guardian, true, false);

      const config = await env.program.account.config.fetch(env.config);
      assert.isTrue(config.mintPaused);
      assert.isFalse(config.redeemPaused);

      await expectError(
        env.program.methods
          .mintRequest(new BN(50), new BN(1_000_000_000), new BN(0))
          .accountsPartial({
            user: env.user.publicKey,
            config: env.config,
            request: env.mintRequestPda(env.user.publicKey, 50),
            collateralMint: env.collateralMint,
            userCollateral: env.userCollateral,
            collateralVault: env.collateralVault,
            oracle: ORACLE_HEALTHY,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([env.user])
          .rpc(),
        "MintPaused"
      );

      // The exit stays open. That asymmetry is the entire reason for two flags.
      const nonce = 51;
      await env.program.methods
        .redeemRequest(new BN(nonce), new BN(1_000_000), new BN(0))
        .accountsPartial({
          user: env.user.publicKey,
          config: env.config,
          request: env.redeemRequestPda(env.user.publicKey, nonce),
          syntheticMint: env.syntheticMint,
          userSynthetic: env.userSynthetic,
          redeemEscrow: env.redeemEscrow,
          oracle: ORACLE_HEALTHY,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([env.user])
        .rpc();

      await sleep(2_000);
      await env.program.methods
        .redeemConfirm(new BN(nonce), PROOF_HASH(61), VENUE_VELOCITY, new BN(1_000_000))
        .accountsPartial({
          keeper: env.keeper.publicKey,
          config: env.config,
          keeperAccount: env.keeperAccount,
          request: env.redeemRequestPda(env.user.publicKey, nonce),
          user: env.user.publicKey,
          syntheticMint: env.syntheticMint,
          redeemEscrow: env.redeemEscrow,
          collateralMint: env.collateralMint,
          collateralVault: env.collateralVault,
          userCollateral: env.userCollateral,
          oracle: ORACLE_HEALTHY,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([env.keeper])
        .rpc();
    });

    it("refuses to let the guardian unpause", async () => {
      await expectError(setPaused(env.guardian, false, false), "GuardianCannotUnpause");
    });

    it("lets the authority unpause", async () => {
      await setPaused(env.payer, false, false);
      const config = await env.program.account.config.fetch(env.config);
      assert.isFalse(config.mintPaused);
      assert.isFalse(config.redeemPaused);
    });
  });

  // -------------------------------------------------------------------------
  describe("issuance gates: carry, capacity, venue state, hard band", () => {
    const reportHealthy = () =>
      env.program.methods
        .reportVenueState(VENUE_VELOCITY, VENUE_CARRY_BPS, VENUE_CAPACITY)
        .accountsPartial({
          signer: env.payer.publicKey,
          config: env.config,
          keeperAccount: null,
        })
        .rpc();

    const tryMint = (nonce: number, oracle = ORACLE_HEALTHY) =>
      env.program.methods
        .mintRequest(new BN(nonce), new BN(1_000_000_000), new BN(0))
        .accountsPartial({
          user: env.user.publicKey,
          config: env.config,
          request: env.mintRequestPda(env.user.publicKey, nonce),
          collateralMint: env.collateralMint,
          userCollateral: env.userCollateral,
          collateralVault: env.collateralVault,
          oracle,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([env.user])
        .rpc();

    const confirm = (
      nonce: number,
      venueId: number,
      filled: BN,
      hash = PROOF_HASH(71)
    ) =>
      env.program.methods
        .mintConfirm(new BN(nonce), hash, venueId, filled)
        .accountsPartial({
          keeper: env.keeper.publicKey,
          config: env.config,
          keeperAccount: env.keeperAccount,
          request: env.mintRequestPda(env.user.publicKey, nonce),
          user: env.user.publicKey,
          syntheticMint: env.syntheticMint,
          userSynthetic: env.userSynthetic,
          oracle: ORACLE_HEALTHY,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([env.keeper])
        .rpc();

    it("lets an active bonded keeper report venue state", async () => {
      // The natural caller is the delta-keeper daemon. Requiring the admin key
      // here would hand that daemon set_params, set_paused, set_oracle and
      // transfer_authority as well -- and make `poyz keeper run` mean "become
      // an admin".
      await env.program.methods
        .reportVenueState(VENUE_VELOCITY, VENUE_CARRY_BPS, VENUE_CAPACITY)
        .accountsPartial({
          signer: env.keeper.publicKey,
          config: env.config,
          keeperAccount: env.keeperAccount,
        })
        .signers([env.keeper])
        .rpc();

      const config = await env.program.account.config.fetch(env.config);
      assert.equal(config.lastNetCarryBps, VENUE_CARRY_BPS);
      assert.notEqual(config.venueStateAt.toString(), "0");
    });

    it("rejects a venue report from an account that is neither authority nor keeper", async () => {
      await expectError(
        env.program.methods
          .reportVenueState(VENUE_VELOCITY, VENUE_CARRY_BPS, VENUE_CAPACITY)
          .accountsPartial({
            signer: env.outsider.publicKey,
            config: env.config,
            keeperAccount: null,
          })
          .signers([env.outsider])
          .rpc(),
        "NotAuthorizedReporter"
      );
    });

    it("clamps a keeper's capacity claim to the admin ceiling", async () => {
      // The asymmetry that makes keeper reporting safe. A false carry report is
      // caught later and slashed; a false *capacity* report would already have
      // minted synthetic dollars that no slash can unmint. So the claim is
      // clamped to a number only the authority sets.
      const inflated = MAX_REPORTABLE_CAPACITY.muln(10);
      await env.program.methods
        .reportVenueState(VENUE_VELOCITY, VENUE_CARRY_BPS, inflated)
        .accountsPartial({
          signer: env.keeper.publicKey,
          config: env.config,
          keeperAccount: env.keeperAccount,
        })
        .signers([env.keeper])
        .rpc();

      const config = await env.program.account.config.fetch(env.config);
      assert.equal(
        config.venueCapacityNotional.toString(),
        MAX_REPORTABLE_CAPACITY.toString(),
        "stored capacity is the ceiling, not the claim"
      );

      // Understating is always allowed: it only tightens issuance.
      await env.program.methods
        .reportVenueState(VENUE_VELOCITY, VENUE_CARRY_BPS, new BN(1))
        .accountsPartial({
          signer: env.keeper.publicKey,
          config: env.config,
          keeperAccount: env.keeperAccount,
        })
        .signers([env.keeper])
        .rpc();
      const tightened = await env.program.account.config.fetch(env.config);
      assert.equal(tightened.venueCapacityNotional.toString(), "1");

      await reportHealthy();
    });

    it("separates the measured carry regimes at the runway floor", async () => {
      // The live SOL delta-neutral carry is negative: the short leg pays
      // funding. The floor is -(3 % buffer / 30 days runway) annualised, and
      // the measured regimes fall on either side of it -- so this is the gate
      // deciding, on real numbers, whether the protocol may keep issuing.
      const config0 = await env.program.account.config.fetch(env.config);
      assert.equal(config0.minNetCarryBps, REFERENCE_MIN_NET_CARRY_BPS);

      // 1y at -35.8 %: inside the runway, issuance continues.
      await env.program.methods
        .reportVenueState(VENUE_VELOCITY, MEASURED_CARRY_1Y, VENUE_CAPACITY)
        .accountsPartial({
          signer: env.payer.publicKey,
          config: env.config,
          keeperAccount: null,
        })
        .rpc();
      await tryMint(59);
      await confirm(59, VENUE_VELOCITY, new BN(152_340_000), PROOF_HASH(70));

      // 30d at -43.3 %: the buffer would not last the required runway, so the
      // protocol stops issuing rather than selling a loss.
      await env.program.methods
        .reportVenueState(VENUE_VELOCITY, MEASURED_CARRY_30D, VENUE_CAPACITY)
        .accountsPartial({
          signer: env.payer.publicKey,
          config: env.config,
          keeperAccount: null,
        })
        .rpc();
      await expectError(tryMint(60), "CarryBelowFloor");

      // Redemption is untouched throughout: a bad regime is exactly when
      // people must be able to leave.
      const config = await env.program.account.config.fetch(env.config);
      assert.isFalse(config.redeemPaused);

      await reportHealthy();
    });

    it("refuses to issue beyond the hedgeable venue capacity", async () => {
      await env.program.methods
        .reportVenueState(VENUE_VELOCITY, VENUE_CARRY_BPS, new BN(1_000_000))
        .accountsPartial({
          signer: env.payer.publicKey,
          config: env.config,
          keeperAccount: null,
        })
        .rpc();

      await expectError(tryMint(61), "VenueCapacityExceeded");

      await reportHealthy();
    });

    it("refuses to issue once the venue state goes stale", async function () {
      this.timeout(60_000);
      await env.program.methods
        .setParams(updateParams({ maxVenueStateAgeSec: 1 }))
        .accountsPartial({ authority: env.payer.publicKey, config: env.config })
        .rpc();

      await sleep(2_500);
      await expectError(tryMint(62), "VenueStateStale");

      await env.program.methods
        .setParams(
          updateParams({ maxVenueStateAgeSec: PARAMS.maxVenueStateAgeSec })
        )
        .accountsPartial({ authority: env.payer.publicKey, config: env.config })
        .rpc();
      await reportHealthy();
    });

    it("refuses to issue when the book is outside the hard band", async () => {
      // Same feed, same freshness, double the price. No token moves; the
      // collateral leg simply revalues away from the attested short.
      await env.program.methods
        .setOracle(FEED_ID)
        .accountsPartial({
          authority: env.payer.publicKey,
          config: env.config,
          oracle: ORACLE_SHIFTED,
        })
        .rpc();

      await expectError(tryMint(63, ORACLE_SHIFTED), "DeltaOutsideHardBand");

      await env.program.methods
        .setOracle(FEED_ID)
        .accountsPartial({
          authority: env.payer.publicKey,
          config: env.config,
          oracle: ORACLE_HEALTHY,
        })
        .rpc();

      // Back at the original price the book is balanced again and issuance
      // resumes without any operator intervention.
      await tryMint(64);
      await confirm(64, VENUE_VELOCITY, new BN(152_340_000));
    });

    it("rejects a mint confirm naming a venue that is not enabled", async () => {
      await tryMint(65);
      await expectError(
        confirm(65, VENUE_JUPITER_PERPS, new BN(152_340_000)),
        "VenueNotEnabled"
      );
      await confirm(65, VENUE_VELOCITY, new BN(152_340_000));
    });

    it("rejects an over-reported hedge fill", async () => {
      await tryMint(66);
      // Over-reporting inflates hedged_notional, which would make the book read
      // as over-hedged and hide a real under-hedge from every band check.
      await expectError(
        confirm(66, VENUE_VELOCITY, new BN(300_000_000)),
        "HedgeFillTooLarge"
      );
      await confirm(66, VENUE_VELOCITY, new BN(152_340_000));
    });
  });

  // -------------------------------------------------------------------------
  describe("boundaries", () => {
    it("rejects a deposit too small to mint one synthetic base unit", async () => {
      await expectError(
        env.program.methods
          .mintRequest(new BN(40), new BN(1), new BN(0))
          .accountsPartial({
            user: env.user.publicKey,
            config: env.config,
            request: env.mintRequestPda(env.user.publicKey, 40),
            collateralMint: env.collateralMint,
            userCollateral: env.userCollateral,
            collateralVault: env.collateralVault,
            oracle: ORACLE_HEALTHY,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([env.user])
          .rpc(),
        "ZeroAmount"
      );
    });

    it("enforces the synthetic supply cap, then mints once it is raised", async function () {
      this.timeout(60_000);
      const nonce = 41;
      const config = await env.program.account.config.fetch(env.config);

      await env.program.methods
        .setParams(updateParams({ maxSyntheticSupply: config.totalSynthetic.addn(1) }))
        .accountsPartial({ authority: env.payer.publicKey, config: env.config })
        .rpc();

      await env.program.methods
        .mintRequest(new BN(nonce), new BN(1_000_000_000), new BN(0))
        .accountsPartial({
          user: env.user.publicKey,
          config: env.config,
          request: env.mintRequestPda(env.user.publicKey, nonce),
          collateralMint: env.collateralMint,
          userCollateral: env.userCollateral,
          collateralVault: env.collateralVault,
          oracle: ORACLE_HEALTHY,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([env.user])
        .rpc();

      const confirm = () =>
        env.program.methods
          .mintConfirm(new BN(nonce), PROOF_HASH(51), VENUE_VELOCITY, new BN(152_340_000))
          .accountsPartial({
            keeper: env.keeper.publicKey,
            config: env.config,
            keeperAccount: env.keeperAccount,
            request: env.mintRequestPda(env.user.publicKey, nonce),
            user: env.user.publicKey,
            syntheticMint: env.syntheticMint,
            userSynthetic: env.userSynthetic,
            oracle: ORACLE_HEALTHY,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([env.keeper])
          .rpc();

      await expectError(confirm(), "SupplyCapExceeded");

      await env.program.methods
        .setParams(updateParams({ maxSyntheticSupply: PARAMS.maxSyntheticSupply }))
        .accountsPartial({ authority: env.payer.publicKey, config: env.config })
        .rpc();

      await confirm();
    });

    it("refuses to cancel a mint request before its deadline", async () => {
      const nonce = 42;
      await env.program.methods
        .mintRequest(new BN(nonce), new BN(1_000_000_000), new BN(0))
        .accountsPartial({
          user: env.user.publicKey,
          config: env.config,
          request: env.mintRequestPda(env.user.publicKey, nonce),
          collateralMint: env.collateralMint,
          userCollateral: env.userCollateral,
          collateralVault: env.collateralVault,
          oracle: ORACLE_HEALTHY,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([env.user])
        .rpc();

      await expectError(
        env.program.methods
          .mintCancel(new BN(nonce))
          .accountsPartial({
            user: env.user.publicKey,
            config: env.config,
            request: env.mintRequestPda(env.user.publicKey, nonce),
            collateralMint: env.collateralMint,
            collateralVault: env.collateralVault,
            userCollateral: env.userCollateral,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([env.user])
          .rpc(),
        "RequestNotExpired"
      );

      // Clean up through the keeper path so no escrow is left behind.
      await env.program.methods
        .mintConfirm(new BN(nonce), PROOF_HASH(53), VENUE_VELOCITY, new BN(152_340_000))
        .accountsPartial({
          keeper: env.keeper.publicKey,
          config: env.config,
          keeperAccount: env.keeperAccount,
          request: env.mintRequestPda(env.user.publicKey, nonce),
          user: env.user.publicKey,
          syntheticMint: env.syntheticMint,
          userSynthetic: env.userSynthetic,
          oracle: ORACLE_HEALTHY,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([env.keeper])
        .rpc();
    });

    it("returns escrowed collateral once the request has expired", async function () {
      this.timeout(60_000);
      const nonce = 43;

      await env.program.methods
        .setParams(updateParams({ requestTtlSec: 1 }))
        .accountsPartial({ authority: env.payer.publicKey, config: env.config })
        .rpc();

      await env.program.methods
        .mintRequest(new BN(nonce), new BN(1_000_000_000), new BN(0))
        .accountsPartial({
          user: env.user.publicKey,
          config: env.config,
          request: env.mintRequestPda(env.user.publicKey, nonce),
          collateralMint: env.collateralMint,
          userCollateral: env.userCollateral,
          collateralVault: env.collateralVault,
          oracle: ORACLE_HEALTHY,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([env.user])
        .rpc();

      await sleep(3_000);

      const before = await tokenBalance(env.provider, env.userCollateral);
      await env.program.methods
        .mintCancel(new BN(nonce))
        .accountsPartial({
          user: env.user.publicKey,
          config: env.config,
          request: env.mintRequestPda(env.user.publicKey, nonce),
          collateralMint: env.collateralMint,
          collateralVault: env.collateralVault,
          userCollateral: env.userCollateral,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([env.user])
        .rpc();

      const after = await tokenBalance(env.provider, env.userCollateral);
      assert.equal(after - before, 1_000_000_000n);

      const config = await env.program.account.config.fetch(env.config);
      assert.equal(config.pendingCollateral.toString(), "0");

      await env.program.methods
        .setParams(updateParams({ requestTtlSec: PARAMS.requestTtlSec }))
        .accountsPartial({ authority: env.payer.publicKey, config: env.config })
        .rpc();
    });

    it("keeps the collateral vault balance equal to the tracked totals", async () => {
      const config = await env.program.account.config.fetch(env.config);
      const vault = await tokenBalance(env.provider, env.collateralVault);
      assert.equal(
        vault,
        BigInt(config.totalCollateral.add(config.pendingCollateral).toString()),
        "vault holds exactly backing plus escrow"
      );
    });
  });

  // -------------------------------------------------------------------------
  describe("insurance buffer", () => {
    const BUFFER_DEPOSIT = new BN(50_000_000); // 50 pUSD

    it("accepts a permissionless deposit", async () => {
      await env.program.methods
        .bufferDeposit(BUFFER_DEPOSIT)
        .accountsPartial({
          depositor: env.user.publicKey,
          config: env.config,
          syntheticMint: env.syntheticMint,
          depositorSynthetic: env.userSynthetic,
          bufferVault: env.bufferVault,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([env.user])
        .rpc();

      const config = await env.program.account.config.fetch(env.config);
      assert.equal(
        config.bufferBalance.toString(),
        String(EXPECTED_TO_BUFFER + BUFFER_DEPOSIT.toNumber())
      );
    });

    it("stays locked while funding is positive", async () => {
      await expectError(
        env.program.methods
          .bufferWithdraw(new BN(1_000_000))
          .accountsPartial({
            authority: env.payer.publicKey,
            config: env.config,
            syntheticMint: env.syntheticMint,
            bufferVault: env.bufferVault,
            fundingVault: env.fundingVault,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .rpc(),
        "BufferLocked"
      );
    });

    it("stays locked until the negative regime has persisted", async () => {
      // Recording a negative carry window moves no tokens at all. That is the
      // whole reason the regime clock lives in `report_venue_state`.
      await env.program.methods
        .reportVenueState(VENUE_VELOCITY, -30, VENUE_CAPACITY)
        .accountsPartial({
          signer: env.payer.publicKey,
          config: env.config,
          keeperAccount: null,
        })
        .rpc();

      const config = await env.program.account.config.fetch(env.config);
      assert.notEqual(config.negativeFundingSince.toString(), "0");
      assert.equal(config.lastNetCarryBps, -30);

      await expectError(
        env.program.methods
          .bufferWithdraw(new BN(1_000_000))
          .accountsPartial({
            authority: env.payer.publicKey,
            config: env.config,
            syntheticMint: env.syntheticMint,
            bufferVault: env.bufferVault,
            fundingVault: env.fundingVault,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .rpc(),
        "BufferLocked"
      );
    });

    it("caps a single draw and routes it to the funding vault", async () => {
      await env.program.methods
        .setParams(updateParams({ bufferUnlockDelaySec: 0 }))
        .accountsPartial({ authority: env.payer.publicKey, config: env.config })
        .rpc();

      const before = await env.program.account.config.fetch(env.config);
      const cap = before.bufferBalance.muln(PARAMS.bufferMaxDrawBps).divn(10_000);

      await expectError(
        env.program.methods
          .bufferWithdraw(cap.addn(1))
          .accountsPartial({
            authority: env.payer.publicKey,
            config: env.config,
            syntheticMint: env.syntheticMint,
            bufferVault: env.bufferVault,
            fundingVault: env.fundingVault,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .rpc(),
        "BufferDrawCapExceeded"
      );

      const fundingBefore = await tokenBalance(env.provider, env.fundingVault);
      await env.program.methods
        .bufferWithdraw(cap)
        .accountsPartial({
          authority: env.payer.publicKey,
          config: env.config,
          syntheticMint: env.syntheticMint,
          bufferVault: env.bufferVault,
          fundingVault: env.fundingVault,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .rpc();

      const fundingAfter = await tokenBalance(env.provider, env.fundingVault);
      assert.equal(fundingAfter - fundingBefore, BigInt(cap.toString()));

      const after = await env.program.account.config.fetch(env.config);
      assert.equal(
        after.bufferBalance.toString(),
        before.bufferBalance.sub(cap).toString()
      );
      assert.isTrue(
        after.accFundingPerShare.gt(before.accFundingPerShare),
        "the draw is distributed to stakers, not to an address the authority picks"
      );

      // ... and the staker can actually claim it.
      const claimBefore = await tokenBalance(env.provider, env.userSynthetic);
      await env.program.methods
        .claimFunding()
        .accountsPartial({
          owner: env.user.publicKey,
          config: env.config,
          position: env.stakePositionPda(env.user.publicKey),
          syntheticMint: env.syntheticMint,
          fundingVault: env.fundingVault,
          ownerSynthetic: env.userSynthetic,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([env.user])
        .rpc();
      const claimAfter = await tokenBalance(env.provider, env.userSynthetic);
      assert.equal(claimAfter - claimBefore, BigInt(cap.toString()));
    });

    it("holds principal through the unstake cooldown, then releases it", async function () {
      this.timeout(60_000);
      const unstakeAccounts = {
        owner: env.user.publicKey,
        config: env.config,
        position: env.stakePositionPda(env.user.publicKey),
        syntheticMint: env.syntheticMint,
        stakeVault: env.stakeVault,
        ownerSynthetic: env.userSynthetic,
        tokenProgram: TOKEN_PROGRAM_ID,
      };

      await expectError(
        env.program.methods
          .unstake()
          .accountsPartial(unstakeAccounts)
          .signers([env.user])
          .rpc(),
        "NoPendingUnstake"
      );

      await env.program.methods
        .requestUnstake(STAKE_AMOUNT)
        .accountsPartial(unstakeAccounts)
        .signers([env.user])
        .rpc();

      // Principal stops earning the moment the exit is requested, so a staker
      // cannot sit out the cooldown and still collect.
      const mid = await env.program.account.config.fetch(env.config);
      assert.equal(mid.totalStaked.toString(), "0");
      assert.equal(
        await tokenBalance(env.provider, env.stakeVault),
        BigInt(STAKE_AMOUNT.toString()),
        "principal is still escrowed during the cooldown"
      );

      await expectError(
        env.program.methods
          .unstake()
          .accountsPartial(unstakeAccounts)
          .signers([env.user])
          .rpc(),
        "UnstakeCooldownActive"
      );

      await sleep(3_000);

      const before = await tokenBalance(env.provider, env.userSynthetic);
      await env.program.methods
        .unstake()
        .accountsPartial(unstakeAccounts)
        .signers([env.user])
        .rpc();
      const after = await tokenBalance(env.provider, env.userSynthetic);

      assert.equal(after - before, BigInt(STAKE_AMOUNT.toString()));
      assert.equal(await tokenBalance(env.provider, env.stakeVault), 0n);
    });
  });
});
