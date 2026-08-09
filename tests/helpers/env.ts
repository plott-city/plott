/**
 * Shared setup for the localnet suite.
 *
 * Everything here runs against a `solana-test-validator` started by
 * `anchor test`. No cluster is ever contacted: `Anchor.toml` pins
 * `cluster = "localnet"`, the payer is the validator's genesis mint, and the
 * Pyth accounts are genesis fixtures.
 */

import * as anchor from "@coral-xyz/anchor";
import { BN } from "@coral-xyz/anchor";
import {
  createAccount,
  createMint,
  getAccount,
  mintTo,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import * as fs from "fs";
import * as path from "path";

import type { Poyz } from "../../target/types/poyz";

// `anchor test` runs the `[scripts] test` command from the Anchor.toml
// directory, so the package root is the working directory. Asserted rather
// than assumed, because every path below depends on it.
export const PACKAGE_ROOT = process.cwd();
if (!fs.existsSync(path.join(PACKAGE_ROOT, "Anchor.toml"))) {
  throw new Error(
    `tests must run from the anchor-program package root, got ${PACKAGE_ROOT}`
  );
}

function readJson<T>(relative: string): T {
  return JSON.parse(fs.readFileSync(path.join(PACKAGE_ROOT, relative), "utf8")) as T;
}

export interface FixtureManifest {
  publishTime: number;
  price: string;
  expo: number;
  feedIdHex: string;
  payer: string;
  feeds: {
    pyth_sol_usd_healthy: string;
    pyth_sol_usd_wide_conf: string;
    pyth_sol_usd_partial: string;
    pyth_sol_usd_shifted: string;
  };
}

export const manifest = readJson<FixtureManifest>("tests/fixtures/manifest.json");
export const idl = readJson<Poyz>("target/idl/poyz.json");

export const FEED_ID: number[] = Array.from(Buffer.from(manifest.feedIdHex, "hex"));
export const ORACLE_HEALTHY = new PublicKey(manifest.feeds.pyth_sol_usd_healthy);
export const ORACLE_WIDE_CONF = new PublicKey(manifest.feeds.pyth_sol_usd_wide_conf);
export const ORACLE_PARTIAL = new PublicKey(manifest.feeds.pyth_sol_usd_partial);
/// Same feed and same health, double the price. Moves the book without moving
/// a single token.
export const ORACLE_SHIFTED = new PublicKey(manifest.feeds.pyth_sol_usd_shifted);

// `state::VENUE_*`. Cross-package contract, fixed in `_DIRECTION.md` 8-1 and
// published as `idl/venues.json`. 1-based: id 0 is "unset", never a venue, so a
// zeroed byte cannot be misread as the primary venue.
export const VENUE_NONE = 0;
export const VENUE_VELOCITY = 1;
export const VENUE_JUPITER_PERPS = 2;
export const VENUE_ADRENA = 3;
export const VENUE_FLASH_TRADE = 4;
export const VENUE_SIMULATED = 255;
/// Bit n enables venue id n; bit 0 is unusable.
export const VENUE_FLAGS_VELOCITY_ONLY = 0b0000_0010;

export interface VenueContract {
  idBase: number;
  unsetId: number;
  maxAssignableId: number;
  venues: Record<string, number>;
  aliases: Record<string, string>;
  retired: Record<string, string>;
  venueFlagsMask: number;
  defaultVenueFlags: number;
}

/** The published contract every off-chain package reads. */
export const venueContract = readJson<VenueContract>("idl/venues.json");

/// Reported hedgeable capacity used in the suite: 1,000,000 pUSD, so the
/// capacity ceiling only binds in the test that deliberately shrinks it.
export const VENUE_CAPACITY = new BN("1000000000000");
export const VENUE_CARRY_BPS = 50;

// `state::REFERENCE_MIN_NET_CARRY_BPS`, and the measured SOL delta-neutral
// carry regimes the floor has to separate. Annualised bps.
export const REFERENCE_MIN_NET_CARRY_BPS = -3_650; // -(3 % buffer / 30 d) * 365
export const MEASURED_CARRY_1Y = -3_580; // -35.8 %/yr -- passes by 0.7 points
export const MEASURED_CARRY_30D = -4_330; // -43.3 % -- refused

/// Admin ceiling on any reported capacity. A reporter may claim less; it cannot
/// claim more. Set above VENUE_CAPACITY so only the clamp test exercises it.
export const MAX_REPORTABLE_CAPACITY = new BN("2000000000000");

// `state::SLASH_REASON_*`, mirroring the enumerated faults in
// `docs/security.md` 2.4.
export const SLASH_REASON_DELTA_OUT_OF_BAND = 1;
export const SLASH_REASON_CARRY_ANOMALY = 2;
export const SLASH_REASON_CAP_BREACH = 3;
export const SLASH_REASON_LIVENESS = 4;
export const SLASH_REASON_FALSE_PROOF = 5;

export const COLLATERAL_DECIMALS = 9; // SOL / LST
export const SYNTHETIC_DECIMALS = 6; // pUSD
export const BOND_DECIMALS = 6; // $POYZ

/** Protocol parameters used by the suite. */
export const PARAMS = {
  feedId: FEED_ID,
  // Set to the guardian keypair's public key in `setupEnv`.
  guardian: PublicKey.default,
  // The ceiling the program allows. The fixtures are stamped ~30 s before the
  // run, and a cold `anchor build` in front of the suite can take minutes;
  // pinning the bound at the maximum keeps the happy path independent of build
  // time while the staleness test drops it to 1 s deliberately.
  maxPriceAgeSec: 3_600,
  maxConfBps: 100, // 1.00 %; the healthy fixture sits at 5 bps
  collateralRatioBps: 10_000, // 1.00x -- delta-neutral, not overcollateralized
  mintFeeBps: 10,
  redeemFeeBps: 10,
  deltaBandBps: 100, // trigger band: rebalance is required beyond 1.00 %
  deltaExitBps: 25, // inner target: a proof must land inside 0.25 %
  deltaHardBps: 300, // emergency band: no issuance at all beyond 3.00 %
  maxHedgeSlippageBps: 50,
  bufferShareBps: 1_000, // 10 % of funding to the insurance buffer
  bufferMaxDrawBps: 5_000,
  maxSupplyVsCapacityBps: 10_000, // issue at most 100 % of hedgeable capacity
  minKeeperBond: new BN(1_000_000_000), // 1000 $POYZ
  maxSyntheticSupply: new BN("1000000000000"), // 1,000,000 pUSD
  requestTtlSec: 600,
  minSettlementDelaySec: 1,
  unbondCooldownSec: 0,
  bufferUnlockDelaySec: 3_600,
  unstakeCooldownSec: 2,
  maxVenueStateAgeSec: 3_600,
  minNetCarryBps: REFERENCE_MIN_NET_CARRY_BPS,
  maxReportableCapacityNotional: MAX_REPORTABLE_CAPACITY,
  venueFlags: VENUE_FLAGS_VELOCITY_ONLY, // the rest are reserved but disabled
};

export function u64le(value: number | BN): Buffer {
  return new BN(value).toArrayLike(Buffer, "le", 8);
}

export function pda(seeds: (Buffer | Uint8Array)[], programId: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync(seeds, programId)[0];
}

export interface Env {
  provider: anchor.AnchorProvider;
  program: anchor.Program<Poyz>;
  payer: Keypair;

  collateralMint: PublicKey;
  syntheticMint: PublicKey;
  bondMint: PublicKey;

  config: PublicKey;
  collateralVault: PublicKey;
  bondVault: PublicKey;
  bufferBondVault: PublicKey;
  fundingVault: PublicKey;
  bufferVault: PublicKey;
  stakeVault: PublicKey;
  redeemEscrow: PublicKey;

  user: Keypair;
  userCollateral: PublicKey;
  userSynthetic: PublicKey;

  keeper: Keypair;
  keeperBondAta: PublicKey;
  keeperAccount: PublicKey;

  keeper2: Keypair;
  keeper2BondAta: PublicKey;
  keeper2Account: PublicKey;

  guardian: Keypair;
  outsider: Keypair;

  authoritySynthetic: PublicKey;

  keeperPda(owner: PublicKey): PublicKey;
  mintRequestPda(owner: PublicKey, nonce: number): PublicKey;
  redeemRequestPda(owner: PublicKey, nonce: number): PublicKey;
  proofPda(sequence: number): PublicKey;
  stakePositionPda(owner: PublicKey): PublicKey;
}

async function fund(
  provider: anchor.AnchorProvider,
  payer: Keypair,
  to: PublicKey,
  lamports: number
): Promise<void> {
  const tx = new anchor.web3.Transaction().add(
    SystemProgram.transfer({ fromPubkey: payer.publicKey, toPubkey: to, lamports })
  );
  await provider.sendAndConfirm(tx, [payer]);
}

/**
 * Create the three mints, the token accounts, and bootstrap the protocol
 * through `initialize` -> vault creation -> unpause.
 */
export async function setupEnv(): Promise<Env> {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = new anchor.Program<Poyz>(idl, provider);
  const payer = (provider.wallet as anchor.Wallet).payer;
  const programId = program.programId;

  const config = pda([Buffer.from("config")], programId);

  // The synthetic mint authority is the config PDA from the moment the mint
  // exists. `initialize` rejects anything else, so there is never a window in
  // which someone other than the protocol can issue pUSD.
  const collateralMint = await createMint(
    provider.connection,
    payer,
    payer.publicKey,
    null,
    COLLATERAL_DECIMALS
  );
  const syntheticMint = await createMint(
    provider.connection,
    payer,
    config,
    null, // no freeze authority: initialize rejects one
    SYNTHETIC_DECIMALS
  );
  const bondMint = await createMint(
    provider.connection,
    payer,
    payer.publicKey,
    null,
    BOND_DECIMALS
  );

  const user = Keypair.generate();
  const keeper = Keypair.generate();
  const keeper2 = Keypair.generate();
  const guardian = Keypair.generate();
  const outsider = Keypair.generate();
  for (const kp of [user, keeper, keeper2, guardian, outsider]) {
    await fund(provider, payer, kp.publicKey, 5 * anchor.web3.LAMPORTS_PER_SOL);
  }

  const userCollateral = await createAccount(
    provider.connection,
    payer,
    collateralMint,
    user.publicKey
  );
  const userSynthetic = await createAccount(
    provider.connection,
    payer,
    syntheticMint,
    user.publicKey
  );
  const authoritySynthetic = await createAccount(
    provider.connection,
    payer,
    syntheticMint,
    payer.publicKey
  );
  const keeperBondAta = await createAccount(
    provider.connection,
    payer,
    bondMint,
    keeper.publicKey
  );
  const keeper2BondAta = await createAccount(
    provider.connection,
    payer,
    bondMint,
    keeper2.publicKey
  );

  // 100 SOL of collateral for the user, 10000 $POYZ for each keeper.
  await mintTo(
    provider.connection,
    payer,
    collateralMint,
    userCollateral,
    payer,
    100n * 1_000_000_000n
  );
  await mintTo(provider.connection, payer, bondMint, keeperBondAta, payer, 10_000_000_000n);
  await mintTo(provider.connection, payer, bondMint, keeper2BondAta, payer, 10_000_000_000n);

  const env: Env = {
    provider,
    program,
    payer,
    collateralMint,
    syntheticMint,
    bondMint,
    config,
    collateralVault: pda(
      [Buffer.from("collateral_vault"), collateralMint.toBuffer()],
      programId
    ),
    bondVault: pda([Buffer.from("bond_vault")], programId),
    bufferBondVault: pda([Buffer.from("buffer_bond_vault")], programId),
    fundingVault: pda([Buffer.from("funding_vault")], programId),
    bufferVault: pda([Buffer.from("buffer_vault")], programId),
    stakeVault: pda([Buffer.from("stake_vault")], programId),
    redeemEscrow: pda([Buffer.from("redeem_escrow")], programId),
    user,
    userCollateral,
    userSynthetic,
    keeper,
    keeperBondAta,
    keeperAccount: pda([Buffer.from("keeper"), keeper.publicKey.toBuffer()], programId),
    keeper2,
    keeper2BondAta,
    keeper2Account: pda([Buffer.from("keeper"), keeper2.publicKey.toBuffer()], programId),
    guardian,
    outsider,
    authoritySynthetic,
    keeperPda: (owner) => pda([Buffer.from("keeper"), owner.toBuffer()], programId),
    mintRequestPda: (owner, nonce) =>
      pda([Buffer.from("mint_request"), owner.toBuffer(), u64le(nonce)], programId),
    redeemRequestPda: (owner, nonce) =>
      pda([Buffer.from("redeem_request"), owner.toBuffer(), u64le(nonce)], programId),
    proofPda: (sequence) => pda([Buffer.from("proof"), u64le(sequence)], programId),
    stakePositionPda: (owner) =>
      pda([Buffer.from("stake"), owner.toBuffer()], programId),
  };

  await program.methods
    .initialize({ ...PARAMS, guardian: guardian.publicKey })
    .accountsPartial({
      authority: payer.publicKey,
      config: env.config,
      collateralMint,
      syntheticMint,
      bondMint,
      oracle: ORACLE_HEALTHY,
      tokenProgram: TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    })
    .rpc();

  await program.methods
    .initCollateralVault()
    .accountsPartial({
      authority: payer.publicKey,
      config: env.config,
      collateralMint,
      collateralVault: env.collateralVault,
      tokenProgram: TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    })
    .rpc();

  await program.methods
    .initBondVaults()
    .accountsPartial({
      authority: payer.publicKey,
      config: env.config,
      bondMint,
      bondVault: env.bondVault,
      bufferBondVault: env.bufferBondVault,
      tokenProgram: TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    })
    .rpc();

  await program.methods
    .initFundingVaults()
    .accountsPartial({
      authority: payer.publicKey,
      config: env.config,
      syntheticMint,
      fundingVault: env.fundingVault,
      bufferVault: env.bufferVault,
      tokenProgram: TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    })
    .rpc();

  await program.methods
    .initStakeVaults()
    .accountsPartial({
      authority: payer.publicKey,
      config: env.config,
      syntheticMint,
      stakeVault: env.stakeVault,
      redeemEscrow: env.redeemEscrow,
      tokenProgram: TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    })
    .rpc();

  await program.methods
    .setPaused(false, false)
    .accountsPartial({ signer: payer.publicKey, config: env.config })
    .rpc();

  // Nothing can be minted until the venue state exists: the carry floor and the
  // capacity ceiling both read it, and both fail closed while it is unset.
  await program.methods
    .reportVenueState(VENUE_VELOCITY, VENUE_CARRY_BPS, VENUE_CAPACITY)
    .accountsPartial({
      signer: payer.publicKey,
      config: env.config,
      keeperAccount: null, // authority path: no keeper account needed
    })
    .rpc();

  return env;
}

/** Assert an instruction fails with a specific `#[error_code]` variant. */
export async function expectError(
  promise: Promise<unknown>,
  expected: string
): Promise<void> {
  try {
    await promise;
  } catch (err: any) {
    let code: string | undefined;
    if (err instanceof anchor.AnchorError) {
      code = err.error.errorCode.code;
    } else if (Array.isArray(err?.logs)) {
      code = anchor.AnchorError.parse(err.logs)?.error.errorCode.code;
    }
    if (code !== expected) {
      throw new Error(
        `expected error ${expected}, got ${code ?? "(unparsed)"}: ${err?.message ?? err}`
      );
    }
    return;
  }
  throw new Error(`expected error ${expected}, but the instruction succeeded`);
}

/**
 * Assert an instruction fails, matching on the raw message rather than an
 * Anchor error code. Used where the guard lives below Anchor -- an `init` on an
 * account that already exists fails in the system program.
 */
export async function expectFailure(
  promise: Promise<unknown>,
  pattern: RegExp
): Promise<void> {
  try {
    await promise;
  } catch (err: any) {
    const text = `${err?.message ?? ""}\n${(err?.logs ?? []).join("\n")}`;
    if (!pattern.test(text)) {
      throw new Error(`expected a failure matching ${pattern}, got: ${text}`);
    }
    return;
  }
  throw new Error(`expected a failure matching ${pattern}, but it succeeded`);
}

export async function tokenBalance(
  provider: anchor.AnchorProvider,
  account: PublicKey
): Promise<bigint> {
  return (await getAccount(provider.connection, account)).amount;
}

export function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

export const PROOF_HASH = (seed: number): number[] =>
  Array.from({ length: 32 }, (_, i) => (seed + i) % 251 || 1);
