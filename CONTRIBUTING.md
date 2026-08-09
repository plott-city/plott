# Contributing

Thanks for taking a look. This covers what a change is expected to carry, and the two
conventions enforced by CI rather than by review.

## Building

```bash
git clone https://github.com/poyzfi/poyz.git
cd poyz

# Formatting and unit tests need only a Rust toolchain.
cargo fmt --all --check
cargo test

# The on-chain artifact additionally needs the Solana toolchain and anchor-cli 0.31.x.
anchor build
anchor test
```

`anchor build` regenerates `idl/poyz.json`. CI checks that every instruction in the IDL has
a handler in `programs/poyz/src/lib.rs` and the other way round, so a new instruction has to
land with a regenerated IDL in the same change.

## Commit messages

Write a plain sentence describing what the change does.

```
add a hard cap to the per-epoch rebalance count
guard against a stale Pyth update in the redeem path
recompute the delta from reported exposures instead of trusting the keeper
```

**Colon prefixes are rejected by CI.** `feat:`, `fix:`, `chore:`, `docs(sdk):` and anything
else shaped like `word:` or `word(scope):` fails the `commit messages` workflow.
Conventional Commits is a coordination protocol for large teams; this repository does not
use it, and a history carrying both styles reads as machine generated.

Also rejected: emoji, and `Co-authored-by` / `Signed-off-by` trailers.

Check before committing:

```bash
./scripts/check-commit-messages.sh --message "your subject line here"
./scripts/check-commit-messages.sh --range origin/main..HEAD
```

## Conventions

- **No emoji.** Anywhere: code, comments, documentation, commit messages. Use `O` / `X` or
  `PASS` / `FAIL` where a status marker is needed.
- **No stub markers on main.** `todo!()`, `unimplemented!()`, `// TODO`, `// FIXME`, empty
  function bodies. If a path is not finished, leave it out and say so in the documentation.
- **Claims match code.** If the README or a spec says the program does something, the
  instruction has to exist and do it. A change that adds a claim adds the behaviour in the
  same pull request.
- **Numbers are measured, not asserted.** Any figure in documentation needs a reproducible
  measurement or an explicit `estimate` label. `docs/research-notes.md` is the source of
  record for every external fact cited elsewhere; add the citation there when you add the
  claim, and keep the `[FACT]` / `[ASSUMPTION]` / `[VERIFY]` marker honest.
- **Parameter changes carry arithmetic.** Band parameters, routing weights and buffer
  thresholds are argued in the style of `docs/hedge-spec.md`, not asserted.

## Risk language

This protocol earns perpetual funding, a market rate that goes negative. Documentation must
not describe the yield as `risk-free`, `guaranteed`, or `no downside`. `docs/risk-spec.md`
sets the tone: name the failure mode, quantify it where possible, let the reader decide.

## Pull requests

1. One logical change per pull request.
2. `cargo fmt --all --check` clean and `cargo test` green.
3. If an instruction changed, the regenerated `idl/poyz.json` is in the same commit.
4. If behaviour changed, say which document you updated, or why none needed it.
5. Security-relevant findings go through `SECURITY.md`, not a public pull request.

## Layout

| Path | Contents |
| --- | --- |
| `programs/poyz/src/` | The Anchor program |
| `idl/poyz.json` | Generated IDL, the interface the SDK is built from |
| `tests/` | Anchor integration tests |
| `docs/` | Protocol specifications and the research record |
| `scripts/` | Repository policy checks, also run by CI |

The TypeScript SDK and the command line interface live in
[poyzfi/poyz-sdk](https://github.com/poyzfi/poyz-sdk).
