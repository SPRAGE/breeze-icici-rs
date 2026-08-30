# breeze-icici

Async Rust SDK for the ICICI Direct Breeze REST and Socket.IO APIs. It is hermetically verified, not live-qualified or published to crates.io.

## Work here

- Enter `nix develop`; read `README.md` and the audit/architecture docs before changing public or wire behavior.
- Preserve exact compact v1 JSON, including GET/DELETE bodies. Historical v2 is a separate query/header protocol.
- Keep local proof separate from live compatibility, publication, and production readiness.
- Rust 2024, declared MSRV 1.85, Rustls-default REST, optional `rust_socketio`; version `0.0.1`, `publish = false`.

```console
cargo test --test fixture_corpus
cargo test --features sdk-contract --test sdk_contract
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
python .ai/generators/compile.py --check
```

`src/` is implementation; `tests/` is the hermetic contract; `docs/` records protocol/release decisions. `.ai/` is guidance source: do not edit generated adapters, and preserve `.codex/local/` and `.codex/tmp/`.

## Invariants

- Pending clients cannot trade. Market and prohibited Margin/Option Plus mutations are unrepresentable.
- Read retry is opt-in. Mutations get one attempt; `AmbiguousMutation` requires reconciliation.
- Documented GTT is single-leg/cover-OCO; SDK-only plain `oco` is backlog.
- Security-master parsing is reader-only. Streams are bounded; production streaming has no live proof.
- Tests use synthetic credentials and loopback/fakes. Never automate live funds or trading mutations.

Do not add live credentials, probe, publish, or claim MSRV/live/production compatibility without the decisions and evidence required by `docs/IMPLEMENTATION_PLAN.md`.
