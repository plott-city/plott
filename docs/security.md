# POYZ Security

Threat model, keeper bond/slash economics, authority and upgrade structure, oracle safeguards, and
audit/disclosure policy. Assumes the architecture in `architecture.md` (program-owned venue
position, keeper that can trade but never withdraw, on-chain execution proofs) and the risk model in
`risk-spec.md`. Citations resolve to `research-notes.md`; canonical `_DIRECTION.md` 8-1.

Guiding principle: **the most powerful untrusted actor (the keeper) cannot steal, only under-perform
-- and under-performance is bounded, detected, bonded, and recoverable.** But 2026-04 taught the
whole ecosystem a harder lesson: **the most dangerous actor is not the keeper, it is whoever holds
admin/upgrade authority** -- because that is how the venue POYZ hedges on lost $285M. This document
puts admin-key takeover first (section 1.1a), since it is the one realized, catastrophic attack.

---

## 1. Threat model

| actor | can do | cannot do | contained by |
|---|---|---|---|
| Public user | mint, redeem, stake, buffer deposit | affect others' custody | account checks, `has_one` |
| Keeper (delegate/trigger) | place/cancel/resize orders, trigger proofs | withdraw margin or collateral | no-withdraw delegate (Path A) or CPI bounds (Path B); per-epoch bleed cap; bond/slash |
| Liquidator (external) | liquidate the short if it breaches maintenance | act if margin healthy | 2-3x leverage + top-ups (`risk-spec.md` 3) |
| **Venue admin (external)** | **on compromise, seize venue funds -- realized 2026-04** | reach POYZ funds not held at the venue | limit backing at venue, small capacity cap, monitor venue governance (1.1a, `risk-spec.md` 2.1) |
| POYZ admin multisig | set risk params (timelocked), pause, set keeper | move user funds, bypass timelock | m-of-n multisig + timelock, no fund-movement authority |
| POYZ upgrade authority | replace program code | act instantly/anonymously | multisig + timelock + durable-nonce hygiene (1.1a) + sunset plan |
| MEV/searcher | sandwich rebalance orders | force POYZ into bad trades directly | oracle-offset orders, clip splitting, private bundles |

### 1.1a Admin-key / privileged-access takeover -- the realized, top-severity threat
On 2026-04-01 the venue POYZ hedges on (formerly Drift) lost ~$285M **not to a code bug but to
privileged-access capture**: attackers spent months building trust with the team, then used Solana
**durable nonces** to get Security Council members to unknowingly **pre-sign** transactions that
handed over admin control; they then whitelisted a fake collateral token and drained the protocol
in ~12 minutes (`research-notes.md` 1.1). An audit would not have caught it -- the code was fine; the
*signers* were the exploit.

POYZ must defend against exactly this, on two fronts:

- **POYZ's own governance (upgrade + admin multisig):**
  - **No blind signing.** Every multisig signer simulates and human-reviews the *decoded* effect of
    a transaction before signing; hardware wallets with clear-signing only.
  - **Durable-nonce hygiene.** Treat any pre-signed / durable-nonce transaction as hostile by
    default; the multisig policy forbids signing transactions whose nonce account or execution timing
    the signer does not control. Periodically audit for outstanding durable-nonce authorizations.
  - **Timelock as a tripwire.** Because all authority-changing and risk-param transactions are
    timelocked (section 3), a maliciously pre-signed admin action is *visible on-chain before it can
    execute*, giving a guardian window to cancel/pause. Timelock is not just anti-rug; it is the
    anti-durable-nonce backstop.
  - **Least authority + separation.** The upgrade authority cannot move funds; the fund paths are
    program logic or timelocked; the guardian can only pause. No single compromised key drains POYZ.
- **Velocity as a dependency:** a *re-compromise of Velocity's admin* would put POYZ margin held at
  Velocity at risk regardless of POYZ's code. Mitigation is to hold as little there as possible
  (Model A thin margin, `risk-spec.md` 2.6), keep the capacity cap small during private beta, and
  monitor Velocity's governance/security posture as a live risk input.

### 1.1 Malicious or compromised keeper
Because the keeper can trade but **cannot withdraw** -- via a no-withdraw delegate if Velocity
retained the former venue's delegation (`[BLOCKED]`, Path A) or via program-CPI-enforced order bounds (Path B,
`architecture.md` 4) -- the "operator runs off with the money" attack is structurally impossible.
Residual keeper attacks:
- **Liveness fault:** stop committing proofs. Proofs are expected hourly (`hedge-spec.md` 3); a
  missing one is observable on-chain and triggers permissionless keeper replacement (section 3).
- **Adversarial trading:** fill against a colluding counterparty at off-oracle prices, draining the
  subaccount while keeping delta neutral. Contained by the per-epoch **bleed cap** in
  `commit_rebalance_proof` (`abs(net_carry) <= max_epoch_bleed_bps * hedged_notional` unless funding
  explains it), which fails or flags the commit; then slash + replace. In Path B this vector is
  closed at the source (program enforces price bounds before the CPI).
- **False hedge attestation (real, patched case):** `hedged_notional` is the one number the program
  cannot recompute -- it cannot read the venue account cross-program (`architecture.md` 8) -- so it is
  the keeper's *attested* input, fed at `mint_confirm` via `filled_notional`. Over-reporting the fill
  inflates the apparent hedge and would let genuine **under-hedge slip past every delta-band check**.
  Originally `filled_notional` had only a lower bound; the missing upper bound was a live hole, now
  closed with `HedgeFillTooLarge` (a matching `HedgeFillTooSmall` guards the other side). Defense in
  depth: the upper bound; the bond/slash that makes a false attestation costly; and the `venues_hash`
  payload, which anyone can re-derive from the actual venue account (verifier in `packages/sdk-ts`) to
  catch the lie. This is the clearest illustration of why the single trusted input is bonded, bounded,
  and independently verifiable.
- **Griefing / churn:** over-trade or breach caps -- proof-checkable, slashable.

### 1.2 Oracle manipulation
POYZ prices collateral off Pyth (`architecture.md` 6): staleness (`get_price_no_older_than`),
confidence cutoff (reject when `conf/price > max_conf_bps`; prefer EMA), feed pinning (SOL/USD id,
`research-notes.md` 3), and a secondary cross-check for large mint/redeem. Cross-venue prices agreed
within ~1 bp at measurement (`research-notes.md` 3). Never derive value from a manipulable AMM pool
price -- only the Pyth aggregate/EMA. (Velocity's internal oracle set is `[VERIFY]` post-relaunch,
but POYZ does not depend on it.)

### 1.3 MEV / sandwich on rebalancing
Oracle-offset orders tie execution to the oracle; split large clips; private/Jito bundles for
sizable clips; timing jitter. Residual leakage lands in `net_carry`, bounded by the bleed cap.

### 1.4 Reentrancy and CPI safety
Validate the callee program ID on every CPI (Velocity, Jupiter, Pyth, token), verify all PDA
derivations, follow checks-effects-interactions, never trust CPI return data unvalidated, use
`token_interface` for Token-2022 collateral. **Extra care for Velocity:** its SDK aliases
`@coral-xyz/anchor` to `@anchor-lang/core@1.0.1`, which must be isolated from POYZ's real Anchor 0.31
via `peerDependenciesMeta` optional + dynamic import (`research-notes.md` 1.2) -- a dependency-
confusion foothold if mishandled.

### 1.5 Privilege escalation
`has_one` / seed constraints so no user can touch another's `StakePosition`/`KeeperBond`; admin-only
instructions require the admin signer; fund-withdrawal paths are PDA-signed and reachable only via
`redeem`, program `hedge_withdraw`, and timelocked `buffer_withdraw`.

---

## 2. Keeper bond and slash economics

### 2.1-2.2 What the bond covers and sizing
Since the keeper cannot withdraw, the bond covers the **max value extractable before detection-and-
replacement** (adversarial trading, 1.1), not custody. With a proposed per-epoch bleed cap
`max_epoch_bleed_bps` -- a sizing parameter for this analysis, not a field the program
carries today -- hedge notional `N`, detection window `d` epochs (hourly proofs, so small):

```
V_drain ~= d * (max_epoch_bleed_bps/10000) * N ;   bond >= k * V_drain   (k >= 2)
```

`[ASSUMPTION]` `max_epoch_bleed_bps = 50`, `d = 1`, `k = 2` -> `keeper_min_bond >= 1% of hedge
notional`, slashed into the buffer on a proven fault.

### 2.3 Honest limits and the hardening path (now the likely default)
A fixed-fraction bond does not scale to large TVL, so it deters and *partially* compensates; the real
containment is no-withdrawal + bleed cap + hourly detection + permissionless replacement + buffer.
**Path B (program-CPI-enforced order pricing) closes the adversarial-trading vector entirely** and,
because Velocity's delegation support is `[BLOCKED]` in private beta, Path B may be the **v1 default
rather than a v2 hardening**. The bond then only needs to cover liveness faults (far smaller).

### 2.4 Slashable faults (rule-based)
Delta outside band without emergency reason; `net_carry` anomaly beyond the bleed cap unexplained by
funding; exceeding share/turnover caps; missing the liveness window. Each maps to a `keeper_slash`
that re-verifies on-chain evidence rather than trusting the caller.

---

## 3. Authority and upgrade structure

| authority | who | control | notes |
|---|---|---|---|
| Program upgrade | multisig (m-of-n) + timelock | delayed, disclosed, durable-nonce-hardened (1.1a) | biggest centralization risk; sunset plan below |
| Admin (risk params) | multisig | timelocked for band/cap/fee/`carry_floor` changes | cannot move funds |
| Guardian (pause) | smaller multisig / key | fast pause, slow unpause | can only stop actions, never move funds; the timelock tripwire's responder |
| Keeper (delegate/trigger) | rotating, bonded | `keeper_register` then `keeper_bond`; `keeper_unbond` / `keeper_slash` on the way out | replaceable; the `active` flag on the `Keeper` account follows the bond, so there is no separate activation instruction |
| Buffer withdraw | admin | timelocked | prevents quiet draining |

Principles: no single key ever moves user funds; timelock on anything that changes risk economics
(bands, caps, fees, `carry_floor`, capacity) so users can exit first; pausing is fast, unpausing/
loosening is slow.

### 3.1 Upgrade-authority centralization (disclosed, not minimized)
A live upgrade authority can change protocol behavior -- a real, material centralization risk, stated
plainly on the site and in docs, and now doubly salient after 2026-04 showed how privileged access is
actually stolen. Mitigation and sunset: multisig behind a timelock from day one, durable-nonce
hygiene (1.1a), and a committed path to transfer the authority to broader governance and ultimately
narrow or revoke it. Until then, users trust the upgrade multisig -- and are told so.

---

## 4. Oracle safeguards (consolidated)

Staleness (`get_price_no_older_than(max_price_age_sec)`), confidence cutoff (`conf/price >
max_conf_bps` -> reject; prefer EMA), integer fixed-point (`price*10^expo`, no `f64`), pinned SOL/USD
feed id (`research-notes.md` 3), secondary-feed cross-check for large mint/redeem, and pull-model
discipline (the tx that reads a price also posts/re-checks it). POYZ's oracle path does not depend on
Velocity's private-beta internals, so it is not `[BLOCKED]`.

---

## 5. Audit, disclosure, and bug bounty

- **External audit before mainnet.** No mainnet deployment (and no advertised carry) until a
  reputable Solana/Anchor audit of `packages/anchor-program` clears its criticals. But 2026-04 is a
  reminder that **an audit does not cover admin/signer compromise** (the venue's code was fine); the
  durable-nonce governance defenses (1.1a) are a separate, required control. Deployment stays gated on
  explicit user approval per `_DIRECTION.md` / `anchor-lessons.md` (no `anchor deploy` /
  `solana program deploy` without the four-part approval).
- **Verified build + public IDL** (reproducible build; committed IDL) so on-chain matches source.
- **Bug bounty** before/at mainnet, tiered by severity (critical = funds-at-risk paths: mint/redeem
  accounting, PDA-signed withdrawal, proof invariant, oracle gating, `carry_gate`), with safe-harbor
  disclosure.
- **Transparency of degradation.** Every risk-state transition (`risk-spec.md` 1.5; `carry_gate`;
  capacity cap) is on-chain-observable and surfaced in the app; advertised metrics are live values or
  labelled `estimate`, and net carry is shown **with its sign** (`_DIRECTION.md` 8-1).
- **Dependency watch.** Integrated venues are **Velocity (formerly Drift, primary)** and **Jupiter Perps
  (secondary)**. Track both (audits, incidents) and Pyth; **venue compromise/discontinuation is a
  live category, not a tail** -- Velocity (formerly Drift) hacked (2026-04), Zeta (2025-05) and Mango v4 (2025-01) dead
  within 18 months (`research-notes.md` 2). Monitoring must include governance/solvency/regulatory
  signals that precede a failure, and the adapter must let POYZ migrate the hedge without a redeploy.

---

## 6. Pre-mainnet security checklist (gate to launch)

| # | item | status |
|---|---|---|
| 1 | External Anchor audit, criticals resolved | `[REQUIRED]` |
| 2 | Durable-nonce governance defenses live (no blind signing, hardware clear-sign, timelock tripwire) | `[REQUIRED]` -- the 2026-04 lesson |
| 3 | Verified build + committed IDL | `[REQUIRED]` |
| 4 | Proof invariant (delta band) + bleed cap + `carry_gate` tested (localnet only) | `[REQUIRED]` |
| 5 | Keeper cannot withdraw -- Path A delegate OR Path B CPI bounds proven by integration test | `[REQUIRED]` |
| 6 | Oracle staleness/confidence gates tested against injected stale/wide feeds | `[REQUIRED]` |
| 7 | Multisig + timelock live on upgrade and risk params | `[REQUIRED]` |
| 8 | Keeper bond >= sizing (2.2); slash paths tested | `[REQUIRED]` |
| 9 | Bug bounty live; disclosure channel published | `[REQUIRED]` |
| 10 | Velocity `[BLOCKED]` items resolved OR Path B shipped (delegation, program ID, account layout) | `[REQUIRED]` |
| 11 | `@velocity-exchange/sdk` isolated (dependency-alias conflict handled, 1.4) | `[REQUIRED]` |
| 12 | User approval for deployment per `anchor-lessons.md` four-part gate | `[REQUIRED]` |

`PASS` on all twelve is "ready for mainnet." Anything less is `FAIL` and blocks deployment.

---

## Sources

Resolved in `docs/research-notes.md`; canonical `_DIRECTION.md` 8-1. The former venue's doc domains
are dead (NXDOMAIN post-rebrand) and not used. Primary:
[Chainalysis - Drift hack (durable nonce / admin takeover)](https://www.chainalysis.com/blog/lessons-from-the-drift-hack/) ·
[bitcoin.com - Drift hack explained](https://news.bitcoin.com/drift-protocol-hack-2026-what-happened-who-lost-money-and-whats-next/) ·
[Pyth Best Practices](https://docs.pyth.network/price-feeds/core/best-practices) ·
[Pyth Solana pull integration](https://docs.pyth.network/price-feeds/core/use-real-time-data/pull-integration/solana) ·
[Ethena risk case study (counterparty precedent)](https://www.chainargos.com/risks-for-synthetic-stablecoins-ethena-labs-usde-case-study/).
