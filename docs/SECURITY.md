# Security and Trading-Safety Requirements

## Threat model and boundary

The SDK is designed to reduce these failures:

- credentials escaping through formatting, serialization, errors, tracing, URLs, panics, or fixtures;
- signing one body and transmitting another;
- redirects or unsafe endpoint overrides forwarding authenticated requests unexpectedly;
- stale timestamps or automatic duplicate mutations;
- malformed/unbounded upstream data causing panic, memory growth, or false trading state;
- reconnect/subscription bugs hiding stream discontinuity;
- ambiguous network failures being treated as a definitely failed mutation;
- invalid or prohibited market/order combinations reaching the broker.
- downstream code inventing signed endpoint payloads or misclassifying a mutation as a retryable read.

It cannot secure a compromised process, store credentials for the application, register a static IP, coordinate unrelated clients/processes, provide broker-side idempotency, decide trade suitability, or guarantee ICICI availability/data correctness.

## Credentials and redaction

- `AppKey`, `SecretKey`, `SessionToken`, `ApiSession`, and decoded stream components use private secret storage.
- Secret storage zeroizes its owned string on drop; the decoded base64 intermediate is also zeroized.
- Secret types and `Credentials` deliberately do not implement Serde serialization.
- Their `Debug` representations are fixed redaction markers. They do not implement a value-bearing `Display`.
- Errors are bounded and scrubbed against the known app key, secret key, session token, and API session before returning from client operations.
- Signing borrows secret text and never sends the Secret Key.
- Normal tracing never logs headers, bodies, URLs, credentials, response bodies, or stream auth payloads.
- Synthetic fixture values are scanned against copied official placeholders.

The documented browser login URL necessarily contains the App Key as `api_key`. It never contains the Secret Key or session token. Applications should still treat that URL as sensitive operational data and avoid logging it broadly.

Repository examples read credentials only from the process environment and do
not load local files. All local operation/instrument inputs are validated before
authentication or endpoint I/O. Read-only and streaming examples may contact
ICICI only when explicitly run; the mutation example has no client and cannot
send its constructed set-funds, order, square-off, or GTT requests.

Zeroization is defense in depth: clones and allocator/runtime copies can exist, and it does not protect swap, core dumps, a debugger, or a compromised process. Applications remain responsible for secret injection, process permissions, and lifecycle.

## Endpoint and TLS controls

Production defaults are fixed HTTPS URLs. `EndpointSetBuilder` rejects:

- URL userinfo, query strings, or fragments;
- paths that do not end in `/`, which prevents ambiguous URL joining;
- every non-HTTPS endpoint by default.

`allow_insecure_loopback_for_tests()` permits `http` only for literal `localhost`, `127.0.0.1`, or `::1`. It does not disable HTTPS certificate checks and does not permit a non-loopback HTTP proxy. Because custom endpoint sets still receive the configured credentials, callers must reserve this explicit test hook for controlled local servers with synthetic credentials.

Reqwest redirects are disabled. A signed response redirect is returned as an error instead of forwarding authentication headers or changing the target. No normal feature accepts invalid certificates.

REST uses Rustls by default. `native-tls` is optional; if both REST backends are enabled, Rustls is selected. The optional `rust_socketio` 0.6 dependency currently uses native TLS/OpenSSL independently, which is a release-policy item rather than a hidden exception.

## Exact signing invariant

For signed v1 calls, the same immutable compact UTF-8 byte vector is both hashed and attached to the Reqwest body. No later layer reserializes, reorders keys, normalizes Unicode, adds whitespace, drops `{}`, or translates a GET/DELETE body into a query string.

The clock is UTC and injectable for testing. Callers cannot supply `X-Timestamp`, `X-Checksum`, or signing headers through a generic header map. Every safe-read retry obtains a fresh timestamp and signature.

Historical v2 bypasses the v1 signer intentionally and uses its documented query/session-header protocol.

## Mutation safety

- Market orders are not constructible through the public order model.
- Limit and explicit stop-loss constructors require positive quantities/prices and validate stop-loss limit/trigger direction.
- Margin/Option Plus mutation products prohibited by the reviewed notice are not exposed.
- SetFunds, place, modify, cancel, square-off, and GTT mutations get one network attempt even when read retry is enabled.
- Send-phase transport, timeout, protocol/decode, or 5xx failures become `AmbiguousMutation`: the SDK says the outcome is unknown and directs the caller to reconcile broker state before deciding whether to retry.
- No client-generated identifier is represented as broker-supported idempotency.
- `EndpointRequest` is sealed; applications cannot add an arbitrary signed request or choose its retry/rate class.
- Mutation bodies and responses are never logged by the SDK. Optional tracing emits only operation name, attempt number, and mutation flag when an attempt starts.

An application must query the relevant order, GTT, or account state after an ambiguous result. Blind retry is unsafe.

## Deadlines, body bounds, and rate rails

The client defaults to connect, first-byte, and total timeouts of 10, 30, and 60 seconds. The total deadline includes local limiter waits and retry delays. Responses are capped at 1 MiB both through `Content-Length` and incremental chunk accumulation before JSON decoding.

The per-client limiter applies 100 attempts/rolling minute, 5,000 attempts/rolling 24 hours, and 10 combined order mutations/second. These are safety rails, not a guarantee against broker throttling. Separate clients, processes, hosts, and languages need application-level coordination.

## Response and stream hardening

- Response messages/body-derived diagnostics are bounded; full bodies are not returned in errors.
- Numeric compatibility is field-specific. Booleans, arrays, arbitrary strings, non-finite values, and unexplained null/empty values are not coerced to zero.
- Unknown statuses remain unknown and are never assigned a known terminal order meaning.
- Stream decoders check lengths and scalar types before access and are covered by bounded property tests.
- Subscription sets and queues are bounded. Duplicate subscriptions do not consume capacity.
- Queue overflow is visible as `LaggedRequiresReconciliation`; order consumers must refresh authoritative state through REST.
- Reconnect reauthenticates and replays desired subscriptions, but cannot prove the server delivered every event during disconnection.
- Call `shutdown().await` for orderly leave/disconnect; drop cleanup is best-effort.

## Instrument integrity

The core parser reads caller-supplied data only. It selects schema by observed filename and normalized headers, indexes known records by exchange plus complete contract identity, and reports malformed/duplicate rows. It does not use fuzzy matches for orders and does not download or refresh files.

If a download/cache helper is later introduced, its security contract must separately specify approved HTTPS sources, timeout and byte limits, content digest/metadata, atomic replacement, freshness/staleness, permissions, and last-known-good behavior. Those properties are not claimed by the current parser.

## Live-system boundary

The normal suite is hermetic. No live compatibility run has been performed. Before any separately authorized live read-only probe:

1. bind the test to an exact source revision and reviewed dependency lock;
2. verify endpoints and the account's registered static-IP requirements;
3. inject credentials without committing them or exposing shell history;
4. allowlist only the approved read endpoints/stream handshakes with a low serial call budget;
5. scrub outputs and rotate credentials if exposure is suspected.

SetFunds and order/GTT/square-off mutations must never be automated as release probes.

## Security qualification work still open

Version `0.0.1` is distributed under Apache-2.0 as an explicitly experimental preview. Crates.io publication does not close the remaining security work: enforce a dependency/vulnerability policy, review the native-TLS streaming dependency, establish an agreed security-reporting channel, run the declared MSRV, and perform an independent public API/security review before any stable or production-readiness claim.

## Release credential handling

The GitHub publication workflow uses the protected `crates-io` Environment and
reads `CARGO_REGISTRY_TOKEN` only in the final publish step. Pull-request and CI
jobs never reference registry or Breeze secrets. The crates.io token should be
scoped only to publishing updates for `breeze-icici`, given a finite expiry,
and rotated immediately after suspected exposure.

Publication remains manual and tag-bound. Protect `v*` tags from updates and
deletion with a GitHub ruleset, require an environment reviewer, and never use
`pull_request_target` with repository or environment secrets. A dry run and
the complete hermetic suite precede token exposure. The final upload uses
`--no-verify` so build scripts cannot run with the token after the already
verified tree and package have been proven unchanged; the public registry
checksum is verified afterward.
