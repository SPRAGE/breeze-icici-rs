# Test Strategy

## Current acceptance state

The repository has two explicit contract entry points:

| Command | Current local result | Purpose |
|---|---|---|
| `cargo test --test fixture_corpus` | 8 passing tests | Validates the source inventory, 27 REST/five stream fixture corpus, required documents, example inventory/safety boundary, and synthetic-data boundary. |
| `cargo test --features sdk-contract --test sdk_contract` | 101 passing tests | Exercises the public API, exact wires, models, HTTP behavior, rate limits, stream codecs/lifecycle, parser, and compile-fail safety boundaries. |
| `cargo check --all-features --examples` | 8 example targets compile | Type-checks authentication, read-only REST, streaming, and offline mutation-construction usage against the public API. |

The five Trybuild cases have reviewed `.stderr` goldens. They fail for their intended reason: credentials are not serializable, pending clients cannot trade, cash instruments cannot gain expiry fields, market orders are not constructible, and downstream crates cannot implement the sealed signed-request contract.

All normal tests are hermetic. Wiremock tests bind only loopback ports and never contact ICICI. No test has live credentials, changes funds, sends a trade, or opens a production feed.

## Risk-to-test map

| Risk | Cheapest reliable layer | Contract location |
|---|---|---|
| Signature differs from transmitted body | Fixed/prepared request plus loopback HTTP | `contract/auth.rs`, `contract/transport.rs` |
| Secrets leak through formatting or errors | Unit and corpus scans | `contract/auth.rs`, `contract/errors.rs`, `fixture_corpus.rs` |
| Invalid instrument/order reaches transport | Constructor and compile-fail tests | `contract/validation.rs`, `tests/ui/` |
| Downstream code invents a signed request or wrong retry class | Sealed-trait compile-fail test | `tests/ui/custom_endpoint_request_is_sealed.rs` |
| Stop-loss semantics or wire fields drift | Validation plus exact wire | `contract/validation.rs`, `contract/rest_wire.rs` |
| Numeric strings/nulls corrupt values | Fixture/model decoding | `contract/responses.rs` |
| Unknown upstream status gains a false meaning | Forward-compatibility decoding | `contract/responses.rs` |
| Wrong method/path/header/body | One prepared-request contract per operation | `contract/rest_wire.rs` |
| Derivative identity is omitted or reordered | Exact quote and historical-v1 variant bodies | `contract/rest_wire.rs` |
| Invalid option-chain instrument is silently rewritten | Fallible conversion and builder validation | `contract/validation.rs` |
| Historical v2 accidentally uses v1 signing | Prepared request plus loopback HTTP | `contract/rest_wire.rs`, `contract/transport.rs` |
| Mutation duplicates after uncertain write | Loopback request count and delayed response | `contract/transport.rs` |
| Deadline excludes limiter/retry waits | Paused-time and loopback deadline tests | `contract/transport.rs` |
| Client exceeds configured local gates | Paused-time model tests | `contract/rate_limit.rs` |
| Positional stream fields are swapped or pre-normalized | Distinct-value raw frame fixtures | `contract/streaming.rs`, `fixtures/stream_frames.json` |
| Malformed stream data panics | Table and bounded property tests | `contract/streaming.rs` |
| Reconnect loses/duplicates desired state | Deterministic fake Socket.IO | `contract/streaming.rs` |
| Stream memory growth or silent loss | Capacity/lag tests | `contract/streaming.rs` |
| Security-master columns drift | Header/schema fixtures | `contract/instruments.rs` |
| Docs and endpoint inventory diverge | Manifest completeness test | `fixture_corpus.rs` |
| Examples disappear, embed credentials, or dispatch a live mutation | Manifest inventory, source safety scan, and Cargo example compilation | `fixture_corpus.rs`, `examples/` |

## Test layers

### 1. Fixture and completeness corpus

`tests/fixtures/manifest.json` pins the reviewed documentation digest, official SDK commit, endpoint/stream inventory, limits, and required Markdown. The corpus asserts:

- exactly 27 unique REST operations and five stream families;
- one wire contract and one normalized success fixture for each REST operation;
- HTTPS endpoints and expected host classes;
- small examples for every observed security-master schema;
- syntactically valid normalized JSON and bounded malformed/error fixtures;
- no copied documentation credentials, sessions, or account identifiers.

This proves corpus completeness relative to the captured source revision, not that ICICI production has not changed.

### 2. Pure unit, validation, and property contracts

These need no socket or wall clock:

- credential, identifier, money, quantity, date, and instrument validation;
- compact JSON and fixed SHA-256 vectors;
- response number/string/null compatibility;
- HTTP/Breeze error categorization and bounded redaction;
- all observed raw stream layouts, including distinct OHLC prices and the 19-position One Click Equity frame, plus malformed variants;
- all security-master schemas, duplicate policy, and lookup identity;
- local rate-limit state with paused Tokio time.

Stream decoder property coverage runs 512 bounded arbitrary-JSON cases and requires every decoder to return a value/error without panicking.

### 3. Exact prepared-request contracts

The `sdk-contract`/`test-util` hook exposes the method, URL, selected headers, and body after validation/signing but before I/O. It is used for all 27 operations so the suite compares exact compact body bytes, including unusual v1 `GET` and `DELETE` bodies.

This layer also verifies derivative quote/historical-v1 identity fields, the lowercase option-chain path, `exch_code`, explicit stop-loss fields, omitted optionals, GTT leg arrays, auth mode, and historical-v2 query encoding.

### 4. Loopback Reqwest integration

Wiremock proves behavior that a prepared request cannot:

- signed v1 GET/DELETE bodies are actually transmitted;
- pending authentication yields a usable signed client;
- historical v2 sends query/session headers without v1 signing;
- redirects are not followed;
- connect configuration, first-byte phase, total deadline, response-byte limit, and status/envelope classification are enforced;
- the total deadline cancels pending local limiter and retry waits;
- safe-read retry uses fresh timestamp/signature and handles HTTP 408 and bounded `Retry-After`;
- mutation 5xx/timeouts stay at one request and return an unknown-outcome reconciliation error;
- authentication errors redact the browser API session.

The suite asserts external requests/results, not Reqwest implementation details.

### 5. Compile-fail public-surface contracts

Trybuild guards safety properties that runtime tests cannot prove:

- `Credentials` cannot be serialized;
- `BreezeClient<Pending>` has no `trading()` method;
- cash instruments expose no expiry builder;
- `OrderType` has no `Market` variant.
- downstream crates cannot implement `EndpointRequest` for an arbitrary wire payload.

Goldens are reviewed diagnostics. Regeneration is not an automatic acceptance step.

### 6. Deterministic stream lifecycle

The fake transport shares the public handle/subscription semantics with production and tests:

- stream-family and script data-kind validation;
- deduplication and the 2,000 subscription cap;
- one explicit interval per candle connection;
- reconnect reauthentication and exactly-once replay of active desired state;
- unsubscribe leave/removal and no replay of removed subscriptions;
- bounded caller reconnect wait;
- malformed event recovery without closing the stream;
- visible order-event overflow requiring REST reconciliation;
- shutdown leaves, disconnect, and rejection of later subscriptions.

Production `rust_socketio` compilation is useful dependency/API proof. It is not a substitute for a real ICICI handshake.

## Required verification matrix

Run from the development shell:

```console
cargo fmt --all -- --check
cargo test --test fixture_corpus
cargo test --features sdk-contract --test sdk_contract
cargo test --all-features
cargo test --doc --all-features
cargo-nextest nextest run --all-features
cargo check --all-features --examples
cargo clippy --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps
cargo check --no-default-features --features rustls-tls
cargo check --no-default-features --features native-tls
cargo check --no-default-features --features rustls-tls,streaming
cargo check --no-default-features --features native-tls,streaming
nix flake check --no-write-lock-file
python .ai/generators/compile.py --check
```

The current checkout has no approved CI workflow, cargo-deny policy, actual Rust 1.85 run, or live suite. Those are release-qualification items rather than claims hidden inside the local test result.

## Live compatibility protocol (not implemented or authorized)

A future live read-only probe must require account-owner authorization, explicit secret injection, a strict endpoint allowlist, serial/low-budget calls, and sanitized output. It may check CustomerDetails, reads, and stream handshakes. It must never automate SetFunds or any order/GTT/square-off mutation.

Coverage percentage is diagnostic, not an acceptance substitute. Captured-source completeness, local build proof, live compatibility, publication, and production readiness remain separate evidence classes.
