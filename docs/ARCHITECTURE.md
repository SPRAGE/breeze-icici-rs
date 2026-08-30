# Architecture and Public API Contract

## Design principles

1. Preserve the broker's exact wire bytes, including signed JSON bodies on v1 `GET` and `DELETE` requests.
2. Represent invalid instruments, quantities, prices, and order combinations through fallible constructors rather than free-form strings.
3. Keep account-changing and trading methods behind an authenticated typestate and a visibly named mutation service.
4. Preserve unknown response values without assigning them a known trading meaning.
5. Keep credentials out of formatting, serialization, errors, URLs, tracing, and fixtures.
6. Make clocks, endpoints, limits, retries, and stream lifecycle deterministic under hermetic tests.

## Implemented crate layout

```text
src/
  lib.rs                 crate documentation, public modules, root re-exports
  auth.rs                credentials, login URL, API-session and stream-token handling
  client.rs              typestate builder, endpoints, Reqwest execution, deadlines/retries
  clock.rs               injectable UTC clock
  error.rs               stable REST/client error taxonomy
  rate_limit.rs          rolling minute, 24-hour, and order-mutation gates
  request.rs             sealed endpoint request contract and envelope decoding
  signing.rs             exact compact-JSON SHA-256 signing
  services.rs            discoverable domain service facades
  domain/mod.rs          money, identifiers, dates, exchanges, instruments, enums
  account.rs             CustomerDetails, demat, funds, SetFunds, margin
  market.rs              historical v1/v2, quotes, option chain
  portfolio.rs           holdings and positions
  orders.rs              place/detail/list/modify/cancel/square-off/preview
  trades.rs              trade list and detail
  risk.rs                margin and limit-price calculators
  gtt.rs                 GTT list/place/modify/cancel
  instruments.rs         security-master parser, index, and script codes
  streaming/
    mod.rs               public events, subscriptions, handles, and errors
    codec.rs             pure frame decoders
    production.rs        optional Socket.IO transport
    testing.rs           deterministic fake transport
  testing.rs             feature-gated REST contract hooks
```

Endpoint modules own their request and response types. `EndpointRequest` is sealed, so downstream crates can execute SDK requests but cannot invent a signed wire payload or assign it an unsafe retry/rate class. Signing, envelope classification, and third-party transport details stay private. `test-util` exposes only the controlled hooks needed by this repository's black-box contracts.

## REST request flow

```text
typed request
  -> local validation and deterministic serialization
  -> local rolling rate gate
  -> fresh timestamp and exact-byte signing, or historical-v2 query encoding
  -> Reqwest with redirects disabled
  -> bounded body read and HTTP/Breeze envelope classification
  -> endpoint-specific typed response
```

The total request deadline encloses rate-limit waits, every attempt, and retry delays. Local validation occurs before quota is consumed. Each actual attempt consumes the general REST budget; order mutations also consume the combined order gate.

## Client typestate and service discovery

`BreezeClient<Pending>` contains credentials but no signed services. It exchanges the browser-returned `ApiSession` through CustomerDetails:

```rust,ignore
let pending = BreezeClient::builder(credentials).build_pending()?;
let (client, customer) = pending.authenticate(api_session).await?;
```

A previously obtained `SessionToken` can restore an authenticated client:

```rust,ignore
let client = BreezeClient::builder(credentials)
    .session_token(session_token)
    .build()?;
```

`BreezeClient<Authenticated>` exposes both `execute(request)` and these facades:

- `account()` for demat, funds, SetFunds, and margin;
- `market_data()` for historical, quotes, and option chain;
- `orders()` for order reads, preview, and GTT reads;
- `portfolio()` for holdings and positions;
- `trades()` for trade list and detail;
- `risk()` for calculators;
- `trading()` for place, modify, cancel, square-off, and GTT mutations;
- `streaming()` when the `streaming` feature is enabled.

The `trading()` name is an attention boundary, not a permissions system. Broker/account authorization remains external.

## Domain model

`Instrument` has private representation and validated `equity`, `future`, and `option` constructors. Cash instruments cannot acquire expiry/right/strike fields, which is also enforced by a compile-fail contract. Standalone endpoint filters use the known `Exchange` values; broker/account availability is still an API concern and is not inferred as a client-side permission.

- `Money` uses `rust_decimal::Decimal` and emits non-exponent decimal strings.
- `Quantity` is a positive integer; response-only counts can represent zero.
- IDs remain opaque strings even when examples are numeric.
- `DateRange` validates ordering and endpoint-specific historical windows.
- `StockCode` trims, uppercases, and validates the documented ASCII shape.
- Unknown order statuses and selected upstream text values are retained as unknown, never mapped to a known terminal state.

Request fields are private and built through constructors/builders. The public order model contains limit and explicit stop-loss orders; it has no market-order variant. Stop-loss constructors validate the buy/sell limit-to-trigger relationship before I/O. The documented GTT surface includes typed single-leg and cover-OCO constructors; the SDK-only plain `oco` spelling is not inferred into the page contract.

## Authentication and wire protocols

### CustomerDetails

The login helper creates the documented browser URL containing only the App Key. The resulting `ApiSession` is exchanged through unsigned compact JSON with `SessionToken` and `AppKey`. On success, the response session token authenticates the returned client.

### Signed v1

For every normal signed v1 request:

1. Serialize once to compact UTF-8 JSON.
2. Format UTC as `YYYY-MM-DDTHH:MM:SS.000Z`.
3. hash `timestamp || exact_body || secret_key` with SHA-256;
4. emit `X-Checksum: token <lowercase hex>`, `X-Timestamp`, `X-AppKey`, `X-SessionToken`, and JSON content type;
5. send the same body byte slice, including for v1 `GET` and `DELETE`.

No layer reserializes a signed body, and callers cannot inject signing headers.

### Historical v2

Historical v2 is a separate query/header protocol. It sends the documented query keys, including `exch_code`, with `X-SessionToken` and `apikey`; it has no v1 body signature.

### Stream credentials

`SessionToken` decoding requires standard base64, valid UTF-8, and exactly two non-empty `user:token` components. The decoded intermediate and owned stream token are zeroized on drop. Error and debug values never include either component.

## Transport, errors, deadlines, and retry

Production endpoints are fixed HTTPS URLs. An explicit endpoint set rejects userinfo, query strings, fragments, and non-directory paths. Plain HTTP is allowed only when the builder explicitly enables it and every target is loopback, which supports hermetic Wiremock tests without weakening production TLS.

Reqwest follows no redirects. The client defaults to a 10-second connect timeout, 30-second first-byte timeout, 60-second total deadline, and a 1 MiB response limit.

The public `Error` categories are:

- `Validation`, `Authentication`, `PermissionDenied`, and `NotFound`;
- `Timeout { phase }` and `RateLimited { retry_after }`;
- `Api`, `Protocol`, `Decode`, and `Transport`;
- `AmbiguousMutation`, which says the outcome is unknown and requires reconciliation before retry.

Messages are bounded and scrubbed against known credential values. `StreamError` is separate because stream lag, decode, connection, closed-state, and subscription-limit failures have different recovery semantics.

Retry is disabled by default. `RetryPolicy::safe_reads()` performs at most three attempts with bounded exponential delays for retryable transport/timeout failures, HTTP 408/429, and selected 5xx responses. It uses a fresh timestamp/signature and honors `Retry-After` only inside the total deadline. Funds, order, square-off, and GTT mutations are never automatically retried. A send-phase mutation failure is returned as `AmbiguousMutation` because no broker idempotency key is documented.

## Local rate limits

The default per-client safety rails are independent:

- 100 REST attempts per rolling minute;
- 5,000 REST attempts per rolling 24 hours;
- 10 combined order placement/modification/cancellation/square-off attempts per second.

They are local to one cloned client state. They do not coordinate other processes, hosts, clients, or SDKs, and the official source does not establish whether the server's daily window is rolling or calendar-based. The rolling 24-hour interpretation is therefore an explicit conservative local policy, not a claim about broker implementation.

## Streaming

With `streaming`, the client uses Socket.IO constrained to WebSocket transport and the family-specific endpoints/events recorded in the audit:

| Family | Connection/event behavior |
|---|---|
| Quotes/depth/commodity | `livestream`, `stock`, explicit script join/leave |
| Orders | `livefeeds`, `order`, no script subscription |
| One Click F&O | `livefeeds`, `stock`, automatic `one_click_fno` join |
| One Click Equity | `livefeeds`, `stock`, automatic `i_click_2_gain` join |
| OHLCV | Breeze API `/ohlcvstream`, interval channel, explicit script join/leave |

Subscriptions are locally validated, deduplicated, and capped at 2,000. Quote and depth subscriptions require the matching script-code data kind. A candle handle accepts one explicit interval, so callers open another candle handle for another interval.

The production adapter maintains desired subscription state, reconnects with bounded delays/attempts, reauthenticates, and replays each active subscription once. `unsubscribe` sends leave and removes desired state. `shutdown` sends remaining leaves and disconnects; drop is best-effort, so applications should call `shutdown` explicitly.

Events are decoded by pure, bounds-checked codecs. Unknown well-formed data is returned as `StreamEvent::Unknown`; malformed data returns `StreamError::Decode` without panicking or closing the stream. The channel is bounded. Overflow becomes `LaggedRequiresReconciliation`, especially important for orders. `wait_until_reconnected_for` gives callers a bounded wait.

The production adapter is compile-tested and fake-transport tested, not live-handshake qualified. `rust_socketio` 0.6 currently brings native TLS/OpenSSL independently of the REST TLS selection.

## Security masters

`SecurityMaster::parse_file` and `ingest_file` accept caller-provided readers and perform no network or cache I/O. Header names and filename identity select the observed NSE, NFO, BSE, BFO, CDNSE, MCX, and mutual-fund layouts. Known instrument rows are indexed by full identity; malformed rows and deterministic duplicate replacement are reported through diagnostics.

The mutual-fund archive is recognized, but its schema does not produce the same equity/derivative lookup identity and is not invented into one. Downloading, caching, freshness, checksums, and atomic replacement remain application responsibilities until a separately specified helper exists.

## Features and compatibility boundary

| Feature | Contract |
|---|---|
| `rustls-tls` (default) | REST uses Rustls; selected if both REST TLS features are enabled. |
| `native-tls` | Alternate REST TLS backend. |
| `streaming` | Production Socket.IO adapter and its upstream native-TLS dependency. |
| `tracing` | Sanitized attempt-start metadata: operation, attempt, mutation flag. |
| `test-util` | Contract hooks and fake stream support. |
| `sdk-contract` | Repository acceptance target; implies `test-util`. |

Changing exact serialization, validation meaning, default retry/rate/deadline policy, or error categorization is semver-significant. Adding response accessors or preserved unknown values can be additive. Public request fields remain private so upstream additions can be incorporated without exposing invalid struct literals.

Preview version `0.0.1` and `publish = false` deliberately separate this hermetically verified source release from a stable or crates.io claim. MSRV, dependency policy, license, remaining contract hardening, live read-only compatibility, and crates.io publication are release gates described in `IMPLEMENTATION_PLAN.md`.
