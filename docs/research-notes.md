# POYZ Research Notes

> Source of record for every external claim cited in `architecture.md`, `hedge-spec.md`,
> `risk-spec.md`, and `security.md`. Each fact carries the URL it came from. Compiled from live web
> research + real API/`npm`/`dig` cross-checks on 2026-08-09; the venue facts were re-verified after
> the backend team found that the original two-venue premise (both venues now gone) no longer holds.
> Canonical project decision: `_DIRECTION.md` section 8-1.

Legend: `[FACT]` = confirmed against a cited source or a live endpoint. `[ASSUMPTION]` = a design
choice/estimate with stated basis. `[VERIFY]` = plausible but must be confirmed against the named
primary source before it is hard-coded. `[BLOCKED]` = cannot currently be verified (e.g. Velocity is
in private beta and its docs are offline), stated honestly with the reason.

---

## 0. Research deltas (what changed versus prior assumptions)

The reason this research pass was mandatory: nearly every load-bearing premise was stale or wrong.

1. **The primary venue no longer exists under its original name.** The venue (formerly Drift) was exploited for ~$285M on
   2026-04-01 (Solana's second-largest hack) and **rebranded to Velocity DEX on 2026-07-01**. All
   every former-venue domain (the old `.trade` site and its data-API host) now resolves to NXDOMAIN,
   so any old venue-doc citation is a dead link. The live data endpoint is
   `https://data.velocity.exchange`.
   Sources: [Chainalysis - lessons from the Drift hack](https://www.chainalysis.com/blog/lessons-from-the-drift-hack/),
   [The Defiant - Drift rebrands to Velocity](https://thedefiant.io/news/defi/drift-protocol-rebrands-to-velocity-dex-ahead-of-relaunch),
   [CoinDesk - Tether/USDT rescue](https://www.coindesk.com/business/2026/04/16/drift-gets-usd148-million-funding-from-tether-and-partners-as-it-replaces-circle-stablecoin-with-usdt-after-massive-exploit).

2. **Current SOL delta-neutral carry is NEGATIVE.** Measured on Velocity SOL-PERP (the venue's own
   aggregation): 1y -35.8% APR, 30d -43.3%, 24h -105.3% (only 7d is positive, +23.7%). The
   statement "delta-neutral earns funding yield" is **not true at this moment** -- a short pays
   carry today. Source: `https://data.velocity.exchange` (measured 2026-08-09; recorded in
   `_DIRECTION.md` 8-1). This reframes the entire product from "yield" to "carry (signed)."

3. **The venue itself was compromised via admin-key social engineering, not a code bug.** The
   attacker (attributed to Lazarus Group by ZachXBT/Elliptic/TRM) spent months building trust with
   the team, then used Solana **durable nonces** to get Security Council members to pre-sign
   transactions that handed over admin control; they whitelisted a fake token as collateral and
   withdrew $285M in ~12 minutes. This is a threat class POYZ's own governance must defend against.
   Sources: [Chainalysis](https://www.chainalysis.com/blog/lessons-from-the-drift-hack/),
   [bitcoin.com - what happened](https://news.bitcoin.com/drift-protocol-hack-2026-what-happened-who-lost-money-and-whats-next/).

4. **Both order-book alternatives died in 2025.** Zeta ceased operations 2025-05-01; Mango v4 was
   voted into wind-down effective 2025-01-13 after the 2022 Eisenberg ~$110M exploit and a 2024-09
   SEC settlement. Combined with the Velocity (formerly Drift) hack, that is **three Solana perp venues gone or
   breached in ~18 months** -- the core of the honesty positioning. Sources:
   [CoinCodeCap - Zeta](https://coincodecap.com/zeta-review),
   [DL News - Mango wind-down](https://www.dlnews.com/articles/defi/mango-dao-votes-to-shut-down-following-sec-settlement/).

5. **The only live secondary is an LP-pool that charges borrow fee, not funding.** Jupiter Perps
   (80%+ of Solana perp volume) is oracle-priced against the JLP pool and charges an hourly borrow
   fee; a SOL short there costs ~6.14%/yr. It is a cost leg, never a yield source
   (`_DIRECTION.md` 8-1). Source:
   [Jupiter Perps - how it works](https://station.jup.ag/labs/perpetual-exchange/how-it-works).

6. **Velocity's funding mechanics are not the former venue's old "Tier-B 0.125%/hr clamp."** The current
   model (per `_DIRECTION.md` 8-1, measured) is an **annual ~10.95% funding floor + a per-market
   dead zone + Capped Symmetric Funding (the AMM pays asymmetric funding only up to 1/3 of its held
   equity per period)**. The 1/3 cap means **short receipts are bounded below the headline funding
   rate even in positive regimes** -- a yield-model constraint any simulator must honor or it
   overstates returns. Mechanism detail is `[BLOCKED]` on Velocity's offline private-beta docs.

7. **Pyth on Solana is a pull oracle** (`PriceUpdateV2`, post-before-read, `get_price_no_older_than`)
   -- unchanged and now confirmed against a live Hermes call; the SOL/USD feed id and price are
   `[FACT]` (section 3).

8. **Ethena's empirical first-loss buffer runs ~1.7% of supply** ($73M, Jun 2026) -- still the
   industry anchor for buffer sizing (section 4).

---

## 1. Velocity DEX, formerly Drift (primary hedge venue)

### 1.1 The 2026-04 hack and rebrand (why the original venue is gone)
- `[FACT]` 2026-04-01: attacker breached the former venue's multi-sig, stealing ~$285M in ~31 txs over ~12
  minutes by whitelisting a fake collateral token after seizing admin control. Method: months of
  social engineering + Solana **durable nonces** to get Security Council members to pre-sign
  admin-handover transactions. Attribution: Lazarus Group (ZachXBT, Elliptic, TRM), linked to the
  Bybit $1.4B theft. [Chainalysis](https://www.chainalysis.com/blog/lessons-from-the-drift-hack/),
  [bitcoin.com](https://news.bitcoin.com/drift-protocol-hack-2026-what-happened-who-lost-money-and-whats-next/).
- `[FACT]` 2026-07-01: rebrand to **Velocity DEX**; offline 3+ months. Tether extended a ~$127.5M
  credit line (partners ~$20M more) on condition of switching the core dollar asset from USDC to
  **USDT**. Relaunch is leaner, perpetuals-only, USDT-settled; full public relaunch targeted Q3
  2026; currently **private beta**.
  [The Defiant](https://thedefiant.io/news/defi/drift-protocol-rebrands-to-velocity-dex-ahead-of-relaunch),
  [CoinDesk](https://www.coindesk.com/business/2026/04/16/drift-gets-usd148-million-funding-from-tether-and-partners-as-it-replaces-circle-stablecoin-with-usdt-after-massive-exploit).

### 1.2 SDK, client, program (measured 2026-08-09)
- `[FACT]` npm: **`@velocity-exchange/sdk` 0.13.0** (`engines.node >=20`, published 2026-08-04). The
  client class is **`VelocityClient`**; the old client-class symbol does not exist in 0.13.0 types.
  The deprecated legacy SDK (2.163.0-beta.13, `engines.node ^24`) will not install on this Node v22 env.
  [npm @velocity-exchange/sdk](https://www.npmjs.com/package/@velocity-exchange/sdk).
- `[FACT]` **Dependency-conflict landmine:** `@velocity-exchange/sdk` aliases `@coral-xyz/anchor` as
  `npm:@anchor-lang/core@1.0.1`, which collides with the real Anchor 0.31 that
  `packages/anchor-program` uses. It must be integrated via `peerDependenciesMeta` optional +
  dynamic import + a narrow structural interface, never as a plain `dependency` (unpacked 13.4MB /
  1099 files, native `solana-bankrun` binaries in prod deps). Source: `_DIRECTION.md` 8-1.
- `[BLOCKED]` **Mainnet program ID.** The pre-hack program ID (formerly Drift) is deprecated
  (compromised protocol, relaunched) and deliberately not reproduced here. The Velocity program ID
  must be read from `@velocity-exchange/sdk` 0.13.0 constants; it cannot be
  confirmed now because Velocity is in private beta and its docs are not public. Do not hard-code the
  old (formerly Drift) ID.
- `[FACT]` **Only 4 perp markets** on Velocity mainnet: idx 0 `SOL-PERP`, 1 `BTC-PERP`, 2
  `ETH-PERP`, 3 `HYPE-PERP`. The former venue's 40+ markets are gone, which constrains collateral/hedge
  choices. Source: `_DIRECTION.md` 8-1.

### 1.3 Funding mechanics (the yield-model core) -- corrected
- `[FACT / measured]` Current SOL-PERP carry (Velocity's own aggregation, 2026-08-09): 24h
  -0.012013%/hr (-105.3% APR), 7d +0.002704%/hr (+23.7%), 30d -0.004937%/hr (-43.3%), 1y
  -0.004086%/hr (-35.8%). Source: `https://data.velocity.exchange` (`_DIRECTION.md` 8-1).
- `[FACT]` Conversion (verified to reproduce the API's APR): `hourly_pct = fundingRate /
  oraclePriceTwap * 100`; annualize `* 24 * 365.25` (**not 8760**). Check: -0.004086 * 24 * 365.25 =
  -35.82% ~ the reported -35.8%.
- `[FACT / BLOCKED detail]` Velocity funding model = **annual ~10.95% funding floor + per-market
  dead zone + Capped Symmetric Funding**, where the AMM pays asymmetric funding only up to **1/3 of
  its held equity per period**. Consequence: **short receipts are capped below headline funding**, so
  a simulator that ignores the cap overstates yield. Full mechanism spec is `[BLOCKED]` (private-beta
  docs offline). Source: `_DIRECTION.md` 8-1.
- `[FACT]` Funding settles in the collateral/quote asset (now **USDT**), i.e. it is a change in the
  subaccount's quote balance, not a separate token. (Carried from the formerly Drift v2 model; `_DIRECTION.md` 8-1.)
- `[FACT / measured]` **Liquidity is effectively absent:** SOL-PERP OI $7,646, 24h volume $8,118,
  max single order ~1,359 SOL (~$103K). This is not enough depth to hedge synthetic-dollar
  collateral at scale, and it caps mintable supply (section 7, `risk-spec.md`). Source:
  `https://data.velocity.exchange` (`_DIRECTION.md` 8-1).

### 1.4 Risk engine and delegated accounts (trust-model keystone -- now partly blocked)
- `[ASSUMPTION / carried]` Velocity is a rebrand of the formerly Drift v2 codebase, so the
  documented venue mechanisms -- maintenance-margin liquidation, an insurance fund, socialized loss
  when bad debt exceeds it, and auto-deleveraging (ADL) of profitable positions -- are assumed to
  carry over. The former venue's original docs that described these are now offline; the citable
  public record is the venue's own past behavior and the Ethena-class literature (section 4). Treat
  specifics as `[VERIFY]`.
- `[BLOCKED]` **Delegated accounts** (a delegate can trade but not withdraw) were the keystone of
  POYZ's trust model (program owns the subaccount, keeper is a no-withdraw delegate). The pre-rebrand
  venue supported this; whether Velocity 0.13.0 retains it cannot be confirmed in private beta. The design
  assumes it carried over; if it did not, POYZ falls back to program-CPI-enforced order placement
  (the v2 hardening in `security.md` 2.3), which removes the dependency. Marked `[BLOCKED]` honestly.
- `[FACT]` Independent oracle: POYZ values collateral via Pyth directly (section 3), not via the
  venue's internal oracle, so this dependency is unaffected by the venue's private-beta status.

---

## 2. Venue landscape (decided: Velocity primary + Jupiter secondary)

Fixed by `_DIRECTION.md` 8-1 (canonical). Cross-package `venue_id` (u8): `0 = velocity`,
`1 = jupiter-perps`, `2 = adrena` (reserved), `3 = flash-trade` (reserved), `255 = unknown`.

| venue | status | carry model | role |
|---|---|---|---|
| **Velocity DEX** (formerly Drift) | live private beta; hacked 2026-04, relaunching | funding-receiving | **primary; only potential yield leg -- but liquidity ~nil, carry negative now** |
| **Jupiter Perps** | live; LP-pool (JLP), 80%+ share | borrow-fee-paying (~6.14%/yr cost to a short) | **secondary; confirmed cost leg, capacity/redundancy only** |
| Adrena / Flash | live; LP-pool | borrow-fee-paying | candidates, unimplemented |
| Bullet (ex-Zeta L2) | live since 2025-09; separate L2 | unconfirmed | **not implemented v1** (bridge/custody risk) |
| Zeta / Mango v4 | dead | -- | **excluded** |

### 2.1 Zeta Markets -- DEAD
- `[FACT]` Ceased operations 2025-05-01 (pivot to "Bullet" L2). Legacy `@zetamarkets/sdk` 1.64.2 has
  no publish since 2025-09-17. [CoinCodeCap](https://coincodecap.com/zeta-review),
  [Solana Compass](https://solanacompass.com/projects/zeta-markets).

### 2.2 Mango v4 -- DEAD (my earlier "recommended second venue" retracted)
- `[FACT]` Mango DAO voted unanimously (23,347,212 votes) to wind down, effective 2025-01-13, after
  the 2022 Eisenberg ~$110M oracle exploit and a 2024-09-27 SEC settlement ($700k penalty, destroy
  MNGO, delist). TVL ~$210M (2021) -> ~$9M at shutdown. Its domains are dead (the main site NXDOMAIN;
  the old API host is now a domain-sale parking page). [DL News](https://www.dlnews.com/articles/defi/mango-dao-votes-to-shut-down-following-sec-settlement/),
  [The Block](https://www.theblock.co/post/334172/mango-markets-to-wind-down-in-wake-of-sec-settlement-dao-battle).
- Correction: my first pass recommended Mango v4 as the second venue. **Retracted** -- it is dead.

### 2.3 Jupiter Perps -- LIVE (LP-pool) -- confirmed secondary
- `[FACT]` Oracle-priced LP-pool: shorts trade against the JLP pool's stablecoin reserves; prices
  from on-chain oracles (Edge by Chaos Labs primary; Chainlink + Pyth fallback), so no order-book
  slippage but capacity-bounded by pool utilization. Live API confirmed:
  `https://perps-api.jup.ag/v1` (valid paths under `/v1`, `/v2`; OpenAPI at `/v1/docs`).
  [Jupiter Perps how-it-works](https://station.jup.ag/labs/perpetual-exchange/how-it-works),
  [Jupiter Perps API docs](https://perps-api.jup.ag/v1/docs).
- `[FACT]` **No funding rate; hourly borrow fee** = `utilization * hourly_borrow_rate *
  position_size`, compounding, paid to the pool. A SOL short currently costs ~**6.14%/yr**
  (`_DIRECTION.md` 8-1; ~0.0168%/day). A POYZ short **pays** carry here.
  [Jupiter fees](https://support.jup.ag/hc/en-us/articles/18735045234588-What-are-the-fees-associated-with-Jupiter-Perps).
- `[ASSUMPTION / DESIGN]` Modeled as an insurance premium POYZ pays to stay hedged when Velocity
  capacity binds or is degraded; never a yield source (`hedge-spec.md` 4). `[VERIFY]` per-token borrow
  schedule + JLP short capacity before sizing the overflow leg.

### 2.4 Bullet (ex-Zeta L2) -- excluded v1
- `[ASSUMPTION / DESIGN]` Bullet (Zeta's successor, separate L2 since ~2025-09) is excluded: hedging
  there means bridging collateral/margin off Solana L1, adding bridge + off-L1 custody risk to the
  exact funds backing $POYZ. Revisit only if L1 hedge capacity becomes the binding constraint and a
  trust-minimized bridge is proven. [CoinCodeCap - Zeta/Bullet](https://coincodecap.com/zeta-review).

---

## 3. Pyth oracle (collateral valuation) -- confirmed

- `[FACT]` Pull model: post `PriceUpdateV2` on-chain before reading; reads use
  `get_price_no_older_than(max_age)` (reverts on staleness). Anchor: `Account<'info, PriceUpdateV2>`
  from `pyth_solana_receiver_sdk`. [Pyth Solana pull integration](https://docs.pyth.network/price-feeds/core/use-real-time-data/pull-integration/solana),
  [Pyth pull oracle launch](https://www.pyth.network/blog/pyth-network-pull-oracle-on-solana).
- `[FACT]` Fixed-point `decimal = price * 10^expo`, same exponent for price and confidence; make
  logic depend on confidence and/or use the EMA price (down-weights wide-confidence samples).
  [Pyth best practices](https://docs.pyth.network/price-feeds/core/best-practices).
- `[FACT]` **SOL/USD feed id** `ef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d`
  (promoted from `[VERIFY]` to `[FACT]` -- confirmed via live Hermes call, `_DIRECTION.md` 8-1).
  Packages: `@pythnetwork/pyth-solana-receiver` (TS), `pyth-sdk-solana` (Rust).
  [pyth-solana-receiver](https://www.npmjs.com/package/@pythnetwork/pyth-solana-receiver).
- `[FACT]` **Cross-venue price agreement (2026-08-09):** Pyth $76.285 / Velocity $76.293 / Jupiter
  $76.294 -- within ~1 bp. Basis between POYZ's oracle value and venue execution is small in calm
  conditions but must still be managed (`hedge-spec.md` 7). Source: `_DIRECTION.md` 8-1.

---

## 4. Ethena USDe (reference design + risk precedents)

- `[FACT]` Synthetic dollar via delta-neutral hedging (long spot + short perp); yield from funding
  paid by leveraged longs plus staking yield. [Ethena - How USDe Works](https://docs.ethena.fi/how-usde-works).
- `[FACT]` sUSDe (ERC-4626) distributes revenue to stakers; a portion is retained in the Reserve
  Fund as first-loss; 7-day unstake cooldown. Reserve/insurance ~1.7% of supply ($73M, Jun 2026;
  started ~$11.8M). [Ethena reserve fund](https://mirror.xyz/0xF99d0E4E3435cc9C9868D1C6274DfaB3e2721341/8Tz63GOVYJE81BI899GIvjMcVlrSn6oGg7Vueyp_iXA),
  [ChainArgos risk case study](https://www.chainargos.com/risks-for-synthetic-stablecoins-ethena-labs-usde-case-study/).
- `[FACT]` Documented Ethena-class risks: sustained negative funding depletes the reserve and can
  impair the peg; exchange counterparty risk (FTX precedent; Bybit hack early 2025 showed
  off-exchange settlement transfers, not eliminates, counterparty risk); staking-collateral slashing
  breaks the hedge ratio. The reserve is "a buffer, not a guarantee."
  [ChainArgos](https://www.chainargos.com/risks-for-synthetic-stablecoins-ethena-labs-usde-case-study/).
- `[ASSUMPTION]` POYZ vs Ethena: POYZ hedges **on-chain**, removing CEX custody/counterparty risk
  but substituting on-chain venue risks -- including, as 2026-04 proved, **venue admin-key takeover**
  -- plus far shallower liquidity than a CEX book. A different, not smaller, risk surface.

---

## 5. Funding-regime history -- now resolved with primary data

- `[FACT / RESOLVED]` The prior open task ("no primary SOL funding series") is closed: Velocity
  provides 31-day raw + 1-year aggregate funding. Measured SOL-PERP carry is **negative on the 30d
  and 1y windows** (-43.3% / -35.8% APR), positive only on 7d (section 1.3). Negative carry is the
  **current regime**, not a hypothetical. Source: `https://data.velocity.exchange` (`_DIRECTION.md` 8-1).
- `[FACT]` Historical context (BTC/ETH): negative funding was frequent through the 2022-2023 bear;
  post-FTX (Nov 2022) BTC funding stayed negative ~46-50 days before shorts capitulated.
  [Phemex - 46 days negative](https://phemex.com/blogs/bitcoin-funding-rates-negative-46-days-ftx-bottom),
  [CryptoRank](https://cryptorank.io/news/feed/d3396-bitcoin-funding-rate-lowest-2023).
- `[ASSUMPTION / MATH]` Annualize from the correct interval and venue cadence. Velocity is hourly:
  `APR = hourly_pct * 24 * 365.25`. (A secondary source once annualized -0.01%/8h as -3.65%/yr by
  treating the 8h figure as daily; correct is ~-10.95%/yr. Do the arithmetic per-venue, never copy.)

---

## 6. Delta-neutral hedging theory (inputs to hedge-spec)

- `[FACT]` Two-band hysteresis (hedge when delta drift exceeds an upper band, unwind below a lower band)
  prevents flapping; threshold rebalancing captures most of the benefit at far fewer trades;
  practical bands ~5-10% of notional. [LuxAlgo](https://www.luxalgo.com/blog/how-delta-hedging-automation-works/),
  [Wundertrading](https://wundertrading.com/journal/en/delta-neutral-strategy),
  [Thetix](https://thetix.ai/blog/delta-neutral-rebalancing-basics-investors).
- `[ASSUMPTION]` POYZ "delta" = (collateral notional minus short perp notional) / collateral
  notional; target 0; symmetric band. Because both legs are linear, delta is the notional imbalance
  (no convexity). Band values proposed (not proven) in `hedge-spec.md`; tune against SOL vol +
  Velocity depth before mainnet.

---

## 7. Cross-package contracts (ground truth = the compiled IDL)

Source of truth for on-chain names is the compiled IDL `packages/anchor-program/target/idl/poyz.json`
(**30 instructions** -- the message-stated 29 omitted `report_venue_state`). The build IDL is the
naming authority; where an earlier draft disagreed, the IDL wins.

- `[FACT / IDL]` Proof instruction is **`commit_rebalance_proof`**. Args (7): `sequence: u64`,
  `venues_hash: [u8;32]`, `venue_id: u8`, `delta_bps_before: i32`, `delta_bps_after: i32`,
  `hedged_notional: u64`, `collateral_notional: u64`. **[RESOLVED]** an earlier `_DIRECTION.md` draft
  named this `commit_execution_proof` with a `proof_hash` arg / "8 args" (the Phase-1 skeleton name);
  the compiled IDL and the finalized `_DIRECTION.md` 8-1 both confirm `commit_rebalance_proof` /
  `venues_hash` / 7 args as canonical. `venues_hash` (keeper-submitted artifact commitment) is kept
  distinct from `this_hash` (program-computed chain hash) so keeper-reported values are verification
  targets, not trusted inputs (`architecture.md` 8).
- `[FACT / IDL]` Mint and redeem are **2-step**: `mint_request(nonce, collateral_amount,
  min_synthetic_out)` -> `mint_confirm(nonce, hedge_proof_hash, venue_id, filled_notional)` /
  `mint_cancel(nonce)`; symmetric `redeem_request` -> `redeem_confirm` / `redeem_cancel`. Minting is
  hedge-first (confirm completes only after the hedge is filled), with the price quoted+locked at
  request time (`MintRequest.quoted_price`, `deadline` from `request_ttl_sec`). Accounts: `Config`,
  `Keeper`, `MintRequest`, `RedeemRequest`, `RebalanceProof`, `StakePosition`.
- `[FACT / CONTRACT]` `venue_id` u8 map: 0 velocity, 1 jupiter-perps, 2 adrena, 3 flash-trade, 255
  unknown. `VenueId` is an extensible string+constant, not a closed union (3 venues died/broke in 18
  months -- hard-coding repeats the mistake).
- `[FACT / IDL]` Carry accounting is a **3-way split**: `gross_funding` (venue funding, signed),
  `hedge_cost` (Jupiter borrow + fees + realized basis), `net_carry = gross_funding - hedge_cost`.
  On-chain it is settled by `settle_funding(amount, funding_rate_bps)`, accumulated into
  `Config.acc_funding_per_share`, and paid by `claim_funding()`. The header metric shows **net carry
  with its sign** (negative as-is), never "funding APY."
- `[FACT / IDL]` `carry_gate` and the capacity cap are **already Config fields**, not just design:
  `min_net_carry_bps` + `last_net_carry_bps` gate mint on carry; `max_supply_vs_capacity_bps` +
  `venue_capacity_notional` + `max_synthetic_supply` gate mint on hedgeable depth;
  `negative_funding_since` drives the playbook. Bands are `delta_band_bps` / `delta_exit_bps` /
  `delta_hard_bps`.

### 7.1 Confirmed constants (measured -- use verbatim)
```
Pyth SOL/USD feed id : ef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d
price cross-check    : Pyth $76.285 / Velocity $76.293 / Jupiter $76.294   (within ~1 bp)
Velocity funding conv: hourly_pct = fundingRate / oraclePriceTwap * 100 ;  APR = hourly_pct * 24 * 365.25
Velocity data API    : https://data.velocity.exchange        (no auth, live)
Jupiter Perps API    : https://perps-api.jup.ag/v1           (no auth, live; docs at /v1/docs)
SDK (use)            : @velocity-exchange/sdk 0.13.0  -> class VelocityClient
SDK (do not advertise): poyz-cli / @poyz/sdk are E404 (unpublished) -> git clone only until published
```

---

## 8. Consolidated citation index (min. 3 per spec doc)

- architecture.md: Velocity/hack + program/SDK (1.1-1.2), Pyth pull (3), Ethena structure (4),
  cross-package contracts (7).
- hedge-spec.md: Velocity funding + VelocityClient (1.2-1.3), delta bands (6), Jupiter borrow-fee
  (2.3), asymmetric funding cap (1.3).
- risk-spec.md: 2026-04 hack (1.1), negative-carry regime (1.3 / 5), Ethena risks (4), venue
  attrition Zeta+Mango (2.1-2.2), Ethena buffer sizing (4).
- security.md: durable-nonce admin takeover (1.1 / 3), Pyth staleness+confidence (3), Ethena
  counterparty precedent (4).
