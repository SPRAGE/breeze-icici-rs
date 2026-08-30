# Project Brief: Breeze ICICI Rust SDK

## Goal

Provide an idiomatic, async Rust client for ICICI Direct's Breeze REST and Socket.IO APIs. Correct reads should be straightforward, trading mutations should be explicit, the broker's unusual signing protocol should be preserved byte-for-byte, and malformed or changing upstream data should become typed errors rather than panics or silent reinterpretation.

## Intended users

- Rust applications reading Breeze market, account, portfolio, order, and trade data.
- Trading systems that need explicit, auditable order and GTT operations.
- Long-running services consuming Breeze Socket.IO feeds with bounded memory.
- Tools resolving instruments and stream script codes from ICICI security-master files.

## Implemented local scope

- Async Reqwest REST client with Rustls by default and an optional native-TLS backend.
- Browser login URL, API-session exchange, restored sessions, exact v1 signing, and separate historical-v2 encoding.
- Typed requests/responses for all 27 operations in the reviewed official reference.
- Account, market-data, order, portfolio, trade, risk, and mutation service facades.
- Cash, future, and option instruments without unrelated placeholder fields.
- Decimal-safe money/prices, positive quantities, UTC/date-range validation, and tolerant documented response adapters.
- Stable validation, authentication, permission, not-found, timeout, rate-limit, API, protocol, decode, transport, and ambiguous-mutation errors.
- Local 100/minute, 5,000/rolling-24-hour, and 10 combined order mutations/second safety rails.
- Optional Socket.IO quotes/depth/commodity, order, One Click, and OHLCV streams.
- Reader-only security-master parsing and full-contract lookup for the observed file families.
- Hermetic fixtures, exact wire contracts, compile-fail safety tests, mocked HTTP behavior, bounded property tests, and deterministic stream lifecycle tests.

## Explicit non-goals

- Strategy execution, signal generation, suitability decisions, or autonomous trading.
- Silent conversion of a requested market order into an aggressive limit order.
- Automated live tests that change funds or place, modify, cancel, or square off orders.
- Credential storage, browser automation, static-IP registration, or account provisioning.
- Implicit security-master downloading or cache management.
- Blind parity with undocumented Python SDK extensions.
- Synchronous/blocking, WASM, or no-std APIs in the current package.
- Treating build/test success as live-broker, publication, or production-readiness proof.

## Data flows

```text
typed request
    -> local validation
    -> compact JSON or historical-v2 query encoding
    -> local rate gate
    -> fresh timestamp + exact-body signature where required
    -> redirect-disabled HTTP transport under one total deadline
    -> bounded Breeze envelope classification
    -> typed response

session token
    -> strict base64 user:token decoding
    -> family-specific Socket.IO connection
    -> validated desired subscription state
    -> bounded event queue
    -> pure protocol decoder
    -> typed StreamEvent, typed decode error, or explicit lag error
```

REST and streaming share credentials and domain types, but their lifecycle, recovery, and backpressure contracts stay separate.

## Implemented technology choices

| Layer | Choice | Boundary |
|---|---|---|
| Language | Rust 2024; declared Rust 1.85 MSRV | The declaration still needs an actual 1.85 verification run. |
| Runtime | Tokio | Async execution, timers, synchronization, and tests. |
| HTTP | Reqwest 0.12 | Redirects disabled; Rustls default, native TLS optional. |
| Serialization | Serde and `serde_json` | Deterministic compact requests and narrow response adapters. |
| Money | `rust_decimal` | Avoid binary floating-point request construction. |
| Time | Chrono | UTC timestamp and broker date/time shapes. |
| Streaming | `rust_socketio` 0.6 behind an adapter | Optional; upstream currently brings native TLS/OpenSSL. |
| Secrets | Private wrappers plus `zeroize` | Redacted formatting/non-serialization and drop-time cleanup. |
| Tests | Wiremock, Proptest, Trybuild, fake Socket.IO | All normal acceptance is synthetic and hermetic. |
| Tooling | Nix, Cargo, rustfmt, Clippy, nextest, Rustdoc | Local reproducibility and verification. |

## Completed milestones

1. Crate foundation, domain primitives, and stable error model.
2. Authentication, signing, endpoints, Reqwest transport, deadlines, rate gates, and opt-in safe-read retry.
3. All documented read-only REST requests and responses.
4. Explicit funds, order, square-off, calculator, and GTT mutations, including stop-loss validation and ambiguous-outcome handling.
5. Security-master parser and instrument/script-code lookup.
6. Pure decoders for every reviewed stream family.
7. Feature-gated production Socket.IO adapter plus deterministic subscription, reconnect, unsubscribe, shutdown, and lag contracts.

Public-release qualification remains a separate milestone because it needs user decisions and external evidence, not more guessed implementation. See `IMPLEMENTATION_PLAN.md`.

## Risks and unresolved external facts

| Risk or unknown | Current treatment |
|---|---|
| Official prose, samples, and maintained SDK disagree. | Follow `DOCUMENTATION_AUDIT.md`, preserve exact reviewed fixtures, and fail closed when a conflict affects trading. |
| Socket.IO server/Engine.IO compatibility may drift. | Adapter compiles and lifecycle is fake-tested; make no live-handshake claim until an authorized sanitized probe exists. |
| GET bodies can be mishandled by intermediaries. | Sign and send the same exact bytes; test through a local HTTP server. |
| Broker numbers alternate among strings, numbers, empty strings, and null. | Use field-specific adapters; never coerce arbitrary shapes to zero. |
| Local limits do not coordinate separate clients/processes. | Document them as per-client safety rails; require an application-level coordinator where needed. |
| Broker-side mutation idempotency is undocumented. | Never automatically retry mutations; return `AmbiguousMutation` after uncertain send-phase failures. |
| Preview/error/empty-result live shapes may differ from repaired samples. | Keep normalized source fixtures and require authorized read-only evidence before a stable release claim. |
| License, semver, and dependency policy are undecided. | Keep preview version `0.0.1` and `publish = false`; the source package name is `breeze-icici`. |

## Decisions fixed by the current contract

- The client is async-only and Rustls-first for REST.
- Requests use private fields with validated constructors/builders; no general raw mutation escape hatch is exposed.
- Market order is not a public order type. Limit and explicit stop-loss orders are supported.
- Safe-read retries are opt-in and bounded. Mutations get one transmission attempt.
- No constructor or parser performs implicit network access.
- Normal tests never use live credentials, feeds, funds changes, or trading mutations.
- Local/build proof, live compatibility, publication, and production trading readiness are reported separately.
