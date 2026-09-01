# Implementation and Release Plan

## Current status

The documentation/tests-first stop gate was lifted by the user and milestones 1–7 have an initial implementation. The selected 27-operation wire and response corpus, compile-fail boundaries, mocked Reqwest behavior, security-master parser, stream codecs, and deterministic stream lifecycle are green locally. The canonical fixtures do not cover every documented request variant; the remaining gaps are tracked in `KNOWN_LIMITATIONS.md`.

This is not the same as a public or production release:

- no live Breeze credential or account was used;
- no live Socket.IO handshake was attempted;
- no funds or trading mutation was sent;
- no crate was published to crates.io;
- the source package is named `breeze-icici` and versioned `0.0.1`;
- license and semver/dependency policies remain user decisions;
- the declared Rust 1.85 MSRV still needs a real 1.85 toolchain run.

`Cargo.toml` therefore uses preview version `0.0.1` and retains `publish = false`.

## Local definition of done

- Every contract in `tests/sdk_contract.rs` is green without ignored tests or placeholder results.
- All 27 documented operations have exact method/path/auth/body contracts and normalized response fixtures.
- V1 signing hashes the exact compact bytes sent, including `GET` and `DELETE` bodies.
- Historical v2 remains a separate query/header adapter.
- Credentials and sessions are redacted from debug/error output and are not serializable.
- Redirects are disabled, production endpoints require HTTPS, responses are byte-bounded, and total deadlines include limiter/retry waits.
- Safe read retry is opt-in and bounded; funds/order/GTT/square-off mutations are never retried automatically.
- Stream decoders are panic-free under bounded property tests; event queues and subscription sets are bounded.
- Reconnect reauthenticates and replays the deduplicated desired subscription set; unsubscribe and shutdown send leaves.
- Parser construction and request construction perform no implicit download or network access.
- Rustfmt, all-feature tests, Clippy with warnings denied, Rustdoc, nextest, and the Nix flake check pass.

## Completed implementation milestones

### 1. Foundation and domain model — complete

Implemented the Rust 2024 library, provisional Rust 1.85 declaration, stable errors, secret wrappers, decimal money, quantities/counts, UTC date ranges, typed cash/future/option instruments, known/unknown status handling, and public feature policy.

Proof: validation, auth, errors, and public-API contract modules.

### 2. Authentication, signing, and HTTP transport — complete locally

Implemented login URL generation, pending/authenticated typestates, CustomerDetails exchange, session restoration, exact compact-JSON signing, HTTPS endpoint validation, no redirects, connect/first-byte/total deadlines, response limits, Breeze envelope classification, local rolling rate gates, sanitized optional tracing, and safe-read retry.

Proof: prepared-request vectors plus Wiremock tests that inspect actual GET/DELETE bodies, signatures, retries, redirect behavior, response limits, and mutation request counts.

### 3. Read-only REST surface — complete locally

Implemented account, margin, historical v1/v2, quotes, option chain, holdings, positions, order list/detail, trade list/detail, preview, and GTT-book requests and response models. Unknown fields are retained where modeled, and unknown order statuses are non-terminal values.

Proof: every normalized success fixture decodes through its request's associated response type.

### 4. Calculators and explicit mutations — complete locally

Implemented SetFunds, margin and limit-price calculators, explicit limit/stop-loss placement, modification, cancellation, square-off, and the documented typed GTT single-leg/cover-OCO requests. Market orders and prohibited margin products are not representable through the normal API. The maintained SDK's undocumented plain `oco` variant remains a parity backlog item.

Proof: wire fixtures for every mutation, builder validation tests, compile-fail market-order test, and one-attempt ambiguous-failure test. No live mutation proof exists or is planned as automation.

### 5. Security-master support — parser complete

Implemented reader-based parsing by normalized header names, deterministic full-contract lookup, duplicate/malformed diagnostics, and script-code parsing for all observed archive filenames.

Proof: schema fixtures, quoted-comma/header variants, duplicate behavior, missing identity columns, and no-network parser tests.

The optional download/cache helper from the original plan was deliberately not added: the core SDK never downloads implicitly, and a secure refresh policy needs a separately scoped cache location, freshness, checksum, and atomic-replacement contract.

### 6. Streaming codecs — complete locally

Implemented strict base64 `user:token` decoding and pure codecs for quote, BSE/NSE depth, commodity, cash/derivative order notifications, One Click F&O plus the raw 19-position One Click Equity frame, and equity/future/option OHLCV layouts with explicit low/high/open/close ordering. Unknown well-formed ticks remain raw.

Proof: documented layout fixtures plus 512-case bounded arbitrary-JSON property coverage.

### 7. Streaming lifecycle — complete hermetically

Implemented a feature-gated production Socket.IO adapter with family-specific endpoints/path/events, WebSocket-only transport, auth payload, bounded reconnect policy, desired-state replay, local subscription validation/deduplication/cap, unsubscribe, clean shutdown, and visible queue lag.

Proof: production feature compilation and deterministic fake-transport lifecycle tests. The upstream `rust_socketio` 0.6 transport currently brings native TLS/OpenSSL even when REST uses Rustls; this is documented rather than hidden.

No live handshake claim is made.

## Milestone 8: release qualification — intentionally open

These actions require evidence or choices beyond local implementation and must remain separate:

1. **MSRV:** run the complete relevant suite with Rust 1.85, then keep or raise `rust-version` from evidence.
2. **Dependency policy:** choose license/vulnerability policy and add `cargo-deny`/audit configuration; review the native-TLS streaming dependency.
3. **Public metadata:** approve crate name, license, repository URL, initial version, and semver support policy.
4. **API review:** run a semver/public-surface review after those decisions and before setting `publish = true`.
5. **CI:** add an approved CI workflow covering formatting, default/native/all-feature builds, tests, Clippy, docs, MSRV, and dependency policy.
6. **Live read-only compatibility:** only with account-owner authorization, use secret injection and a strict allowlist to check CustomerDetails/read endpoints and stream handshakes; sanitize and bind the result to an immutable commit.
7. **Publication:** perform only on an explicit request, then verify the registry artifact separately from source/build proof.

Automated live funds/order/GTT/square-off mutations are not a release gate.

## Verification sequence

Run in the Nix development shell:

```console
cargo fmt --all -- --check
cargo test --test fixture_corpus
cargo test --features sdk-contract --test sdk_contract
cargo test --all-features
cargo-nextest nextest run --all-features
cargo check --all-features --examples
cargo clippy --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps
nix flake check --no-write-lock-file
python .ai/generators/compile.py --check
```

Also compile the REST client with each intended TLS configuration:

```console
cargo check --no-default-features --features rustls-tls
cargo check --no-default-features --features native-tls
cargo check --no-default-features --features rustls-tls,streaming
cargo check --no-default-features --features native-tls,streaming
```

Report local/build proof separately from MSRV, live compatibility, publication, and production trading readiness.

## Completion evidence (2026-08-30)

- The official HTML was fetched again and still matched the pinned SHA-256 digest.
- Rustfmt and the default compile completed without warnings.
- Fixture corpus: 7/7 passed.
- SDK contract: 99/99 passed, including five reviewed compile-fail cases.
- `cargo test --all-features`: 7 corpus, 99 SDK, and one Rustdoc test passed.
- Nextest: 106/106 passed.
- All-target/all-feature Clippy passed with warnings denied; all-feature Rustdoc passed with warnings denied.
- Rustls-only, native-TLS-only, and both streaming combinations compiled in the Nix shell.
- `nix flake check --no-write-lock-file` passed for x86_64 Linux evaluation; the command reported aarch64 Linux/Darwin as incompatible systems not checked on this host.
- The project guidance compiler reported all 15 generated files current and within context budgets.

The run used Rust 1.97.1. Rust 1.85 remains a declared but unproven MSRV. The separately prescribed external `ai-doctor` was not executed because the environment rejected downloading and running an unpinned GitHub flake; the local guidance compiler is the available proof. No live or publication evidence was produced.

## Correctness hardening evidence (2026-08-31)

- Distinct-value equity, option, and future OHLCV fixtures prove the documented low/high/open/close positional mapping.
- The One Click Equity fixture and decoder use the maintained Python SDK's raw 19-position Socket.IO frame rather than its post-parse callback object.
- Fallible `TryFrom<Instrument>` option-chain conversion rejects cash, futures, and non-NFO/BFO options; the builder independently rejects unsupported exchanges.
- Exact-wire variant tests prove complete expiry/right/strike identity on derivative quote and historical-v1 requests while preserving the canonical cash request bytes.
- Fixture corpus: 7/7 passed; SDK contract: 101/101 passed; all-feature tests and one Rustdoc test passed; Nextest: 108/108 passed.
- Rustfmt, all-target/all-feature Clippy with warnings denied, all-feature Rustdoc with warnings denied, all four TLS/streaming feature combinations, the guidance compiler, and `nix flake check --no-write-lock-file` passed on x86_64 Linux.
- No live broker connection, credential, funds action, trading mutation, publication, or production claim was introduced.

## Review gates

- Security review: secret formatting, redirect/header behavior, error redaction, response limits, and dependency TLS policy.
- Mutation review: exact serialization, prohibited variants, rate classes, and no retry after possible write.
- Protocol review: Socket.IO paths/auth/events, frame ambiguity, bounded reconnect, replay, and lag semantics.
- Public API review: service discoverability, forward-compatible response values, feature combinations, and semver surface.

Current local evidence includes inline security, mutation, protocol, and public-surface review. An independent review remains a release gate.
