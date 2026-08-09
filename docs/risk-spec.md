# POYZ Risk Specification

This document exists to **surface** risk, not to reassure. Two facts drive everything below and are
stated up front, not buried:

1. **The carry is negative right now.** Measured on Velocity SOL-PERP (the venue's own aggregation,
   2026-08-09): 1y -35.8% APR, 30d -43.3%, 24h -105.3%; only the 7d window is positive (+23.7%)
   (`research-notes.md` 2, 5). A delta-neutral short **pays** carry today. Negative carry is the
   current regime, not a hypothetical scenario.
2. **The primary venue was hacked four months ago.** The venue POYZ hedges on (formerly Drift) was
   exploited for ~$285M on 2026-04-01 and relaunched as Velocity DEX, currently in private beta
   (`research-notes.md` 1.1). Venue risk here is not abstract; it already happened.

This protocol's design and its public copy deliberately avoid the language of guaranteed or
downside-free returns, because none of it would be true. Every projection is an **estimate** on
stated assumptions with the math shown. Ethena's USDe is the closest precedent, and its documented
failure modes -- negative funding depleting the reserve, exchange counterparty loss, staking-
collateral slashing -- are answered here for a Solana, on-chain-hedged design (`research-notes.md` 4).

---

## 1. Negative carry -- the current regime

### 1.1 What it is and why it is here
The short earns funding when perps trade at a premium (longs pay shorts) and **pays** when they
trade at a discount. Discounts appear when the market is net-short: after drops, during
deleveraging, through bear phases. POYZ's carry is therefore **pro-cyclical** and, on the 30d/1y
windows, currently negative (section above). This is the single most important risk fact about the
product, and POYZ responds to it structurally, not rhetorically (1.5, `carry_gate`).

### 1.2 Yield-collapse / buffer-drain math (shown, not asserted)
Supply `S` (USD), fully hedged so hedge notional `H ~= S`. Hourly carry rate to the short `f_h`
(negative in this regime), settling in the quote asset (USDT). Velocity funding is hourly:

```
carry per day = f_h * 24 * H ;   APR = f_h * 24 * 365.25   (verified: -0.004086%/hr -> -35.8% APR)
```

With buffer `B`, `b = B/S`, daily cost `f_d = |f_h| * 24`:

```
buffer runway (days) = B / (|f_h| * 24 * H) = b / f_d
```

### 1.3 Stress table -- baselined to the CURRENT regime
Days the buffer covers the bleed before exhaustion, `= b / f_d`. Columns are the **measured**
Velocity carry regimes, not invented scenarios. Assumes full hedge (`H ~= S`), buffer is the sole
first-loss layer, no new mint. Estimate under those assumptions.

| buffer `b` \ regime | 1y -35.8% APR (baseline) | 30d -43.3% APR | 24h -105.3% APR (spot) |
|---|---|---|---|
| **1.7%** (Ethena-anchored) | 17.3 d | 14.3 d | 5.9 d |
| **3.0%** (POYZ target) | 30.6 d | 25.3 d | 10.4 d |
| **5.0%** | 51.0 d | 42.2 d | 17.3 d |

Reading it: at the **1-year average** regime, a 3% buffer lasts ~**31 days**; at the 30d regime
~25 days; at the 24h spot rate ~10 days. Even the target buffer is weeks, not months -- and this is
before adding Jupiter borrow-fee drag (below) or any correlated stress. A buffer cannot outlast an
indefinite negative regime; it buys time to deleverage, and `carry_gate` stops the protocol from
digging deeper.

**Jupiter overflow shortens the runway.** Velocity's OI is ~$7,646 (`research-notes.md` 1.3), so
almost any real hedge must overflow to Jupiter, where a short pays ~6.14%/yr borrow
(`research-notes.md` 2.3). That cost adds to `f_d`, moving the columns further left. The blended-
carry math is `hedge-spec.md` 4.2.

### 1.4 `carry_gate` -- the structural response
Because minting in a negative-carry regime adds hedge that bleeds the buffer, POYZ refuses it
on-chain: `mint` reverts unless EWMA net carry `>= carry_floor`, where `carry_floor(daily) = -(b /
min_runway_days)` guarantees at least `min_runway_days` of runway at the gated carry
(`architecture.md` 8.1). Worked (`b=3%`, `min_runway_days=30`): floor `~ -36.5%/yr`. Against
measured data, the 1y regime (-35.8%) *just* passes, the 30d (-43.3%) and 24h (-105%) regimes are
**blocked**. As the buffer drains, `b` falls and the floor tightens automatically. This is POYZ's
differentiator: it does not print when printing does not pay.

### 1.5 Negative-carry playbook (buffer-level triggers)
Actions escalate as the buffer drains toward `buffer_target_bps`. Each is a protocol state.

| buffer vs target | actions |
|---|---|
| > 75% | staker reward index floors at ~0 (stakers earn ~0, not negative yet); buffer covers the bleed; `carry_gate` may already block new mint. |
| 50-75% | raise `mint_fee_bps`; trim hedge on the worst-carry venue; raise redemption incentive to shrink supply. |
| 25-50% | actively deleverage (reduce hedge notional -> cut carry cost, reintroduce bounded disclosed delta); `mint` fully paused. |
| < 25% | halt & unwind: queue/encourage redemptions; controlled unwind toward a lower-leverage or fully-collateralized posture; governance decision point. |
| 0% | backing-only: staker NAV declines (subordination); redemption at oracle NAV of remaining collateral; full disclosure. The failure mode, stated plainly. |

Deleveraging is the real defense: reducing the hedge removes carry cost at the price of bounded,
disclosed delta -- chosen over unbounded buffer bleed.

---

## 2. Venue risk -- it already happened

**Venue mortality and compromise are realized risks, not hypotheticals, and this is the strongest
single reason to distrust any "just hedge on a perp DEX" pitch.** In ~18 months, three Solana perp
venues were removed or breached:

- **Velocity (formerly Drift) -- hacked 2026-04-01, ~$285M** (Solana's second-largest hack). The attacker
  socially engineered the former venue's team over months, then used Solana **durable nonces** to get Security
  Council members to unknowingly pre-sign transactions that handed over admin control; they
  whitelisted a fake collateral token and drained $285M in ~12 minutes. Attributed to Lazarus Group
  by ZachXBT/Elliptic/TRM. This is **the venue POYZ hedges on** (now Velocity, in private beta).
  [Chainalysis](https://www.chainalysis.com/blog/lessons-from-the-drift-hack/),
  [bitcoin.com](https://news.bitcoin.com/drift-protocol-hack-2026-what-happened-who-lost-money-and-whats-next/).
- **Zeta -- ceased operations 2025-05-01** (pivot to Bullet L2).
  [CoinCodeCap](https://coincodecap.com/zeta-review).
- **Mango v4 -- wind-down effective 2025-01-13** after the 2022 Eisenberg ~$110M oracle exploit and
  a 2024-09 SEC settlement. [DL News](https://www.dlnews.com/articles/defi/mango-dao-votes-to-shut-down-following-sec-settlement/).

POYZ's answer is not to pretend permanence: the venue-adapter abstraction (`hedge-spec.md` 1), the
in-venue share cap (`hedge-spec.md` 4.1), the capacity-bounded supply (`architecture.md` 11.1), and
limiting backing held at any venue all assume a venue can vanish. Concretely, **venue migration is an
on-chain admin action, not a program upgrade**: `Config.venue_flags` is a bitmask of active venues
(initial Velocity only, `0b0001`), so if Velocity fails or a better venue appears, an operator flips
a flag via `set_params` and routing moves -- no redeploy (`architecture.md` 5.1). In a market where
three venues died or were breached in ~18 months, that configurability is a risk control, not
premature generality. Beyond disappearance, the venue's own failure modes -- inherited from the
formerly Drift v2 codebase Velocity rebranded, so `[VERIFY]` against the relaunched program:

### 2.1 Admin-key takeover (the 2026-04 mechanism) -- highest severity
The 2026-04 hack was not a code bug; it was **privileged-access capture via durable-nonce social
engineering**. Two consequences for POYZ: (a) POYZ's own governance must defend against the same
attack (`security.md` 1.1a); (b) a re-compromise of Velocity's admin would put POYZ margin at that
venue at risk regardless of POYZ's own code. **Mitigation:** limit backing held at the venue
(`hedge-spec.md` 4.1 share cap + Model A thin margin, 2.6), monitor Velocity's governance/security
posture as a live input, and keep the capacity cap small while Velocity is in private beta.

### 2.2 Auto-deleveraging (ADL) -- the most dangerous market mechanism
`[VERIFY / carried]` On the inherited (formerly Drift) codebase, ADL force-closes profitable positions when the insurance
fund cannot absorb a bankrupt one. POYZ's short is *most profitable in a crash* -- prime ADL target
exactly when the hedge is most needed. If fraction `a` of the hedge is ADL'd at price `P` and SOL
drops a further `q` before re-hedge: `loss ~= a * H * q`. Example: `a=0.30`, `q=0.10` -> ~3% of
supply, more than an Ethena-sized buffer. **Mitigations:** multi-venue split (Jupiter is LP-pool, no
ADL); emergency re-hedge funded by the buffer; `delta_hard_bps` pause.

### 2.3 Socialized loss
`[VERIFY / carried]` When a market's bad debt exceeds its insurance fund, the excess is socialized
across users in that market. **Mitigations:** in-venue share cap, keep most backing in the POYZ
vault not at the venue (2.6), monitor the venue insurance fund.

### 2.4 Oracle deviation
POYZ prices collateral off Pyth with staleness + confidence gates (`architecture.md` 6); wide
confidence pauses `mint`/`redeem`. Cross-venue prices agreed within ~1 bp at measurement
(`research-notes.md` 3). Residual: a correct-but-fast price still produces basis (`hedge-spec.md` 7).

### 2.5 Venue down / private beta / chain congestion
Velocity is **in private beta**, so availability and even API/SDK stability are not guaranteed; a
halt or outage stops the keeper from rebalancing and delta drifts unhedged. **Mitigations:**
`delta_hard_bps` breaker + `mint` pause; buffer absorbs the delta drift's P&L; Jupiter as a live fallback;
the capacity cap keeps exposure small until Velocity is production-stable.

### 2.6 Concentration-vs-liquidation (core decision)
- **Model A -- thin margin at venue:** keep most collateral in the POYZ vault, post only a margin
  slice. Limits venue smart-contract / socialized-loss / ADL / admin-takeover exposure to that
  slice; costs active liquidation management (section 3).
- **Model B -- collateral-as-margin:** post collateral itself as venue margin; near-zero liquidation
  risk, but the *entire backing* sits inside one venue -- maximal exposure to the very failure that
  happened in 2026-04.

`[ASSUMPTION / RECOMMENDATION]` POYZ uses **Model A with conservative leverage** (2-3x, section 3):
venue-catastrophe losses are effectively total for exposed funds, while liquidation is manageable
with monitoring. After a $285M admin-takeover on this exact venue, minimizing funds held at the
venue is the clear call.

---

## 3. Liquidation risk (the short's margin slice, Model A)

Margin `M` against short notional `H` (leverage `L = H/M`). A SOL rise of `r` gives short mark loss
`r*H`, equity `M - r*H`; liquidation when equity hits maintenance `m*H`:

```
liquidation move  r* = 1/L - m
```

`[ASSUMPTION]` `m ~= 0.03` for SOL-PERP; `[VERIFY]` against Velocity market specs (private beta).

| short leverage `L` | margin `M/H` | liquidation move `r*` |
|---|---|---|
| 2x | 50% | ~47% |
| 3x | 33% | ~30% |
| 5x | 20% | ~17% |
| 10x | 10% | ~7% |

At 2-3x the short tolerates a 30-47% SOL rally before liquidation, well beyond the keeper's PDA-
signed top-ups (funded by the collateral's own gain during that rally). At 10x a 7% candle
liquidates the hedge -- unacceptable for a dollar. **Rule:** hedge leverage 2-3x, margin utilisation
<= 50%, top up on SOL moves not just ticks.

---

## 4. Depeg and redemption risk

### 4.1 Redemption is the peg mechanism
If $POYZ trades below $1, an arbitrageur buys it cheap and redeems for $1 of collateral at oracle
NAV, shrinking supply and restoring the peg. So a **functioning redemption path is the peg**; the
risk is redemption becoming impaired (venue down, unwind slippage, prior losses leaving backing <
supply).

### 4.2 Mass-redemption unwind slippage -- acute given Velocity's depth
Redeeming `R` forces buying back `R` of short at slippage `s(R)`; realized cost `~ s(R)*R`, borne by
the redeem fee then the buffer. **Velocity's OI is ~$7,646 and max order ~$103K** (`research-notes.md`
1.3), so `s(R)` is severe for any meaningful `R`, and the buy-back mostly falls to Jupiter's pool.
This is *the* reason supply is capped to hedge capacity (`architecture.md` 11.1): a synthetic dollar
you cannot unwind is a synthetic dollar you cannot honor. **Mitigations:** `redeem_fee_bps`, an
unwind queue rate-limited to available depth, multi-venue buy-back, buffer covers residual. **Honest
limit:** in a violent unwind the buffer can exhaust and later redeemers receive oracle NAV of
remaining collateral, which can be below $1 if prior losses impaired backing. Disclosed, not
engineered away.

### 4.3 Collateral-vs-index tracking (LST de-peg)
If collateral is an LST, its price can diverge from the SOL index the perp tracks (de-peg,
slashing), breaking the hedge -- the on-chain analog of Ethena's slashing risk (`research-notes.md`
4). Mitigations: own-feed valuation, SOL-equivalent hedge sizing, LST collateral-share cap
(`hedge-spec.md` 7).

---

## 5. The correlated tail (the honest headline)

The dangerous scenario is the joint event. A sharp SOL crash simultaneously: (a) deepens negative
carry (buffer draining faster, `carry_gate` blocking mint), (b) maximizes ADL pressure on POYZ's
now-deep-in-profit short (hedge force-closed when most needed), (c) drives a flight to dollars = mass
redemptions into Velocity's near-empty book + Jupiter's pool, and (d) can coincide with chain
congestion or -- as 2026-04 proved -- an outright venue compromise. These are positively correlated,
so a "sum of independent small probabilities" understates the true tail. POYZ's defenses -- buffer,
`carry_gate`, capacity cap, multi-venue, low leverage, unwind queue, hard-band mint pause -- are
sized for the joint event, and even so the honest statement is that a sufficiently severe crash-plus-
negative-carry regime degrades backing and can break the peg. No delta-neutral dollar has eliminated
this; POYZ's contribution is to disclose it and make every degradation step a defined, on-chain-
observable state.

---

## 6. Buffer sizing conclusion

- Anchor: Ethena runs a first-loss buffer around 1.7% of supply (`research-notes.md` 4).
- From the current-regime table (1.3), 1.7% buys ~17 days at the 1y regime; a 3% buffer ~31 days.
- `[ASSUMPTION]` Target `buffer_target_bps` = **300 bps (3%)** -- above Ethena's empirical level,
  because the ADL tail (2.2) can consume multiple percent in one event and the venue was just
  hacked. Bootstrap from fees, keeper slashings, and any positive-carry epochs; `mint` is throttled
  (and `carry_gate` may block it) while the buffer is below target. A starting target to be re-
  derived from live data, not a proven safe level. Because carry is negative today, the buffer
  cannot be funded from carry now -- another reason `carry_gate` and the capacity cap keep supply
  small until the regime turns.

---

## 7. Summary risk register

| risk | trigger | primary mitigation | residual (honest) |
|---|---|---|---|
| Venue admin takeover (2.1) | privileged-access social engineering (durable nonce) -- **realized 2026-04, $285M** | limit backing at venue, thin margin, monitor venue governance, small capacity cap | re-compromise of Velocity puts venue-held margin at risk |
| Venue shutdown (2) | discontinuation (Zeta 2025-05, Mango 2025-01 -- realized) | venue-adapter abstraction, capacity cap, share cap | if Velocity winds down, hedge migrates under stress |
| Negative carry (1) | perps at discount -- **current regime, -35.8% APR** | `carry_gate` blocks mint; deleverage playbook (1.5); buffer | indefinite regime > buffer runway degrades backing |
| ADL (2.2) | crash bankrupts venue counterparties | multi-venue (Jupiter has no ADL), emergency re-hedge, hard-band pause | hedge force-closed mid-crash, several % loss |
| Socialized loss (2.3) | venue bad debt > insurance fund | in-venue share cap, thin margin | catastrophic-cascade haircut possible |
| Liquidation (3) | SOL rally bleeds thin margin | 2-3x leverage, keeper top-ups | top-up failure (congestion) loses hedge |
| Redemption unwind (4.2) | mass redemption into a ~$7.6K-OI book | capacity cap, redeem fee, unwind queue, buffer | late redeemers get sub-$1 NAV after buffer |
| LST de-peg (4.3) | staking slash / LST discount | own-feed valuation, SOL-equiv hedge, cap | hedge-ratio break on collateral leg |
| Correlated tail (5) | severe crash + negative carry + venue stress together | all of the above, sized for joint event | peg break under sufficiently severe regime |

---

## Sources

Resolved in `docs/research-notes.md`; canonical `_DIRECTION.md` 8-1. The former venue's doc domains
are dead (NXDOMAIN post-rebrand) and not used. Primary:
[Chainalysis - Drift hack](https://www.chainalysis.com/blog/lessons-from-the-drift-hack/) ·
[CoinDesk - Tether/USDT rescue](https://www.coindesk.com/business/2026/04/16/drift-gets-usd148-million-funding-from-tether-and-partners-as-it-replaces-circle-stablecoin-with-usdt-after-massive-exploit) ·
[DL News - Mango wind-down](https://www.dlnews.com/articles/defi/mango-dao-votes-to-shut-down-following-sec-settlement/) ·
[Ethena risk case study (ChainArgos)](https://www.chainargos.com/risks-for-synthetic-stablecoins-ethena-labs-usde-case-study/) ·
[BTC negative funding history (Phemex)](https://phemex.com/blogs/bitcoin-funding-rates-negative-46-days-ftx-bottom).
