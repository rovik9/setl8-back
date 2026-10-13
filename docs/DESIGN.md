# Setl8 backend design

Setl8 is a prop-trading platform on Solana. This repository is the backend. For each trading sector (first leveraged trading, `lev`; later `options`) a trusted evaluator service turns signed trader orders plus price samples into journal events, and a publisher posts those events to the sector's Solana program. Smart contracts are not developed here.

## Trust model

There is one trusted evaluator per sector, and its output is checked twice: by the on-chain program, and by an independent verifier (`setl8-verif`). The evaluator is trusted to produce events, never to be unchecked.

## Determinism and replay

Everything is deterministic and replayable. The state of an account is a fold over three append-only inputs: signed trader intents, price samples with proofs, and clock ticks. The pure integer engine and evaluator crates come from the sector contract repos through `vendor/`. There is no second implementation of the rules in this repository, so the evaluator that runs here is the same logic the contracts and the verifier rely on.

## Services

**Ingestion** runs as two nodes in different failure domains. It adapts and normalizes price feeds, emits explicit gap events when data is missing instead of papering over it, and publishes a content-addressed archive.

**Gateway** authenticates wallet signatures on orders. It writes the signed intent to a replicated log before acknowledging the trader, so an acknowledged order is never lost.

**Evaluator-service** runs one process per sector, each with its own key and its own database schema. Work is divided into shards held under a lease with a fencing epoch, so a stale holder cannot write after losing the lease. A warm standby takes over when the lease lapses.

**Publisher** batches journal events, at most 8 events and 3 proofs per batch. Each transaction is at most 1,232 bytes and 200k compute units. A batch counts as posted only at finalized commitment, and resends are idempotent.

**Keeper** performs the permissionless heartbeat and `finalize_pass` calls. It holds no funds.

## Signing

Signing never happens in this repository. The publisher sends canonical bytes to `setl8-sign`, which applies policy and signs.

## Fail-safe behavior

Under lag or bad feeds the system refuses risk-increasing orders first. Downtime never fails an account. A suspect price print never automatically fails an account.

## Data

State lives in PostgreSQL. Log tables are insert-only, and each account carries a hash chain (`seq`, `prev_hash`, `hash`) so history is tamper-evident. Services communicate over a message bus; NATS JetStream is proposed.

## Non-goals

This repository does not contain a matching engine, does not take custody of user funds, and does not allow manual overrides of trade outcomes.

## Open items

Event formats, account layouts and wire encodings are defined by the contract repos and arrive through `interfaces/`. Until then they are marked `TODO(interfaces)` and nothing here guesses them.
