# Security Policy

## Reporting a vulnerability

Report privately. Do not open a public issue, and do not describe the finding in a
pull request.

Use GitHub's private vulnerability reporting on this repository:
**Security -> Report a vulnerability**
(<https://github.com/poyzfi/poyz/security/advisories/new>). It creates a private
advisory thread visible only to the maintainers and to you.

If that route is unavailable, send a direct message to
[@poyzfi](https://x.com/poyzfi) asking for a private channel. Do not put finding
details in a public post.

Include what you have: the affected instruction or module, the conditions required,
and what an attacker gains. A proof-of-concept against a local validator is useful
but not required to report.

## What to expect

| Stage | Target |
| --- | --- |
| Acknowledgement of the report | 72 hours |
| Initial assessment and severity | 7 days |
| Fix or documented mitigation for a confirmed high-severity finding | 30 days |
| Public disclosure | after a fix ships, coordinated with the reporter |

If a report goes unacknowledged past 72 hours, ping the same channel. Silence is a
failure on our side, not a rejection.

## Scope

In scope:

- The `poyz` Anchor program under `programs/` -- account validation, arithmetic,
  authority and upgrade paths, oracle handling, keeper bond and slash accounting,
  and the rebalance proof chain.
- The generated `idl/poyz.json` where it misdescribes an instruction in a way that would
  lead a client to build the wrong transaction.
- Documentation in `docs/` where an incorrect specification would lead an integrator
  into an unsafe integration.

The TypeScript SDK and the command line interface are covered by the security policy of
[poyzfi/poyz-sdk](https://github.com/poyzfi/poyz-sdk), not by this one.

Out of scope:

- Findings that require a compromised signer key or a maliciously modified client.
- Denial of service against public RPC endpoints, or third-party infrastructure that
  this project does not operate.
- Market risk. Negative funding, venue outages and liquidation are inherent to the
  design and are documented in `docs/risk-spec.md`. Those are disclosures, not
  vulnerabilities. A defect in how the protocol *handles* one of them is in scope.
- Automated scanner output with no demonstrated impact.

## Deployment status

The program is not deployed to Solana mainnet. `Anchor.toml` targets
`localnet` and `declare_id!` currently holds the Anchor placeholder id
`Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS`. There is no live program to attack
yet, and no funds are at risk today. Reports against the source are still welcome
and are the cheapest time to fix anything.

This project has not been audited. `docs/security.md` sets out the threat model, the
authority and upgrade structure, and the oracle safeguards the program relies on.
Read it before integrating.

## Safe harbour

Good-faith research on a local validator or a private fork is welcome, and we will
not pursue action over it. Do not test against third-party mainnet infrastructure,
do not access data that is not yours, and do not run anything that degrades service
for others.
