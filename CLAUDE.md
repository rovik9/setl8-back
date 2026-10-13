# CLAUDE.md

Setl8 backend (`setl8-back`). Sibling repos: `setl8-sign` (policy signer), `setl8-verif` (independent verifier). Smart contracts are never developed here: no Anchor or program code, no Solana program builds.

## Hard rules

1. No private keys, seed phrases, wallet files or API secrets anywhere: code, tests, fixtures, CI. Tests may only use throwaway keys generated at test runtime.
2. All money, price and PnL math is integer-only. No f32/f64 in any decision path.
3. Engine logic never reads the wall clock. Time arrives as an input event.
4. Never edit interfaces/ or vendor/. They are exported from the contract repos by the owner on his own machine and are read-only for sessions.
5. Never push to main. Work on a branch and open a PR.
6. Every commit is authored and committed as: Rovik <therovikdev@gmail.com>. No Co-Authored-By trailer, no AI attribution, no session-link trailer, in commits or PR bodies.
7. If the spec is silent or contradictory, take the smallest safe option and list it under DECISIONS in the PR. Never invent protocol details (event formats, account layouts); mark them TODO(interfaces).

## Repo map

- `crates/common` shared types placeholder
- `crates/ingestion` price adapters, normalization, gap events, archive writer
- `crates/gateway` API, wallet-signed order intake, durable intent log
- `crates/evaluator-service` hosts sector rules engines, shard leases, standby
- `crates/publisher` batching, tx building, finality tracking, signer client
- `crates/keeper` permissionless heartbeat and finalize bot, holds no funds
- `interfaces/`, `vendor/` read-only exports (see their READMEs)
- `docs/DESIGN.md` architecture
- `scripts/` CI helper checks (identity, protected paths)

## Checks before pushing

`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace && cargo deny check`
