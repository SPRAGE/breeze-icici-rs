# `breeze-icici`

An experimental async, typed Rust SDK for the ICICI Direct Breeze REST and Socket.IO APIs.

Version `0.0.1` is a source-only preview exercised by a hermetic fixture and mock-transport suite. It has **not** been validated against a live Breeze account, published to crates.io, or qualified for unattended production trading. `publish = false` remains intentional. Review the [known limitations](docs/KNOWN_LIMITATIONS.md) before using it against a broker account.

## What is implemented

- Browser login URL generation, CustomerDetails session exchange, and restored sessions.
- Exact v1 compact-JSON SHA-256 signing, including bodies on `GET` and `DELETE`.
- The separate historical-v2 query/header protocol.
- Typed request and response foundations covering all 27 operations on the reviewed reference page; some documented variants remain incomplete.
- Account, market-data, order, portfolio, trade, calculator, funds, square-off, and GTT service facades.
- Decimal prices, positive quantities, typed instruments, local request validation, stable error categories, timeouts, response limits, rate gates, and opt-in safe-read retry.
- Initial Socket.IO quote/depth/commodity, order, One Click, and OHLCV support with bounded queues, reconnect replay, unsubscribe, and explicit lag errors.
- Reader-based security-master parsing for the observed NSE, NFO, BSE, BFO, CDNSE, and MCX records; mutual-fund records are not yet materialized.
- Compile-fail safety contracts: credentials are not serializable, unauthenticated clients cannot trade, cash instruments cannot gain expiry fields, market orders are not constructible, and downstream crates cannot invent arbitrary signed endpoint requests.

The [coverage matrix](docs/API_COVERAGE.md) maps every documented operation and stream to its current request type and canonical contract fixture. Green canonical fixtures do not imply that every documented variant is complete.

## Authentication

Create the login URL, let the account owner complete login in a browser, then exchange the returned API session:

```rust,no_run
use breeze_icici::{
    ApiSession, AppKey, BreezeClient, Credentials, SecretKey, login_url,
};

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let credentials = Credentials::new(
    AppKey::new(std::env::var("BREEZE_APP_KEY")?)?,
    SecretKey::new(std::env::var("BREEZE_SECRET_KEY")?)?,
);

let url = login_url(credentials.app_key())?;
println!("Open this URL in a browser: {url}");

// Supply only the API session returned by the completed browser flow.
let api_session = ApiSession::new(std::env::var("BREEZE_API_SESSION")?)?;
let pending = BreezeClient::builder(credentials).build_pending()?;
let (client, customer) = pending.authenticate(api_session).await?;

println!("authenticated user: {}", customer.user_id().as_str());
let funds = client.account().funds().await?;
println!("unallocated balance: {}", funds.unallocated_balance());
# Ok(())
# }
```

If a valid CustomerDetails session token was persisted by the application, restore it explicitly:

```rust,no_run
use breeze_icici::{AppKey, BreezeClient, Credentials, SecretKey, SessionToken};

# fn example() -> Result<(), Box<dyn std::error::Error>> {
let credentials = Credentials::new(
    AppKey::new(std::env::var("BREEZE_APP_KEY")?)?,
    SecretKey::new(std::env::var("BREEZE_SECRET_KEY")?)?,
);
let client = BreezeClient::builder(credentials)
    .session_token(SessionToken::new(std::env::var("BREEZE_SESSION_TOKEN")?)?)
    .build()?;
# let _ = client;
# Ok(())
# }
```

Credential types redact `Debug`, are not serializable, and are zeroized when their owned storage is dropped. Applications remain responsible for secure secret storage and process-level access control.

## Calling APIs

Validated request values are inert until passed to a service method or `BreezeClient::execute`:

```rust,no_run
use breeze_icici::domain::{Exchange, Instrument, StockCode};
use breeze_icici::market::QuoteRequest;
use breeze_icici::BreezeClient;

# async fn example(client: BreezeClient<breeze_icici::Authenticated>) -> Result<(), Box<dyn std::error::Error>> {
let instrument = Instrument::equity(
    Exchange::Nse,
    StockCode::new("ITC")?,
)?;
let quotes = client.market_data().quotes(QuoteRequest::new(instrument)).await?;
for quote in quotes {
    println!("{}: {}", quote.stock_code(), quote.last_price());
}
# Ok(())
# }
```

Mutations are deliberately grouped under clearly named methods such as `client.trading().place(request)` and `client.trading().cancel(request)`. The public order model has no market-order variant. Funds and trading mutations are never automatically retried after an ambiguous network write.

## Streaming

Enable `streaming` and create a stream from an authenticated client:

```rust,no_run
# #[cfg(feature = "streaming")]
# async fn example(client: breeze_icici::BreezeClient<breeze_icici::Authenticated>) -> Result<(), Box<dyn std::error::Error>> {
use std::str::FromStr;
use breeze_icici::instruments::ScriptCode;
use breeze_icici::streaming::{StreamEvent, StreamKind, Subscription};

let streams = client.streaming()?;
let mut feed = streams.connect(StreamKind::MarketData).await?;
feed.subscribe(Subscription::quote(ScriptCode::from_str("4.1!1594")?)).await?;

while let Some(event) = feed.next_event().await {
    match event? {
        StreamEvent::Quote(quote) => println!("{}", quote.last_price()),
        _ => {}
    }
}
# Ok(())
# }
```

The queue is bounded. A slow consumer receives `LaggedRequiresReconciliation`; order consumers should reconcile through REST rather than assume no state change occurred. Call `shutdown().await` for an orderly leave/disconnect. The production adapter has compile and deterministic fake-transport proof only until a separately authorized live read-only handshake is recorded.

## Cargo features

| Feature | Behavior |
|---|---|
| `rustls-tls` (default) | Rustls for REST HTTPS. If both REST TLS features are enabled, Rustls is selected. |
| `native-tls` | Native TLS alternative for REST. |
| `streaming` | Production Socket.IO adapter. Its current upstream transport uses native TLS/OpenSSL even when REST uses Rustls. |
| `tracing` | Emits sanitized request-attempt metadata: operation, attempt, and mutation flag only. |
| `test-util` | Prepared-request, signing, clock, response, and fake-stream test support. Not intended for normal applications. |
| `sdk-contract` | Enables this repository's black-box acceptance target and `test-util`. |

Security-master parsing is part of the normal API and never downloads or caches files implicitly.

## Development and verification

The Nix shell includes Rust, rustfmt, Clippy, nextest, pkg-config, and OpenSSL:

```console
nix develop
cargo fmt --all -- --check
cargo test --test fixture_corpus
cargo test --features sdk-contract --test sdk_contract
cargo-nextest nextest run --all-features
cargo clippy --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps
nix flake check
```

All normal tests use synthetic credentials and local mock transports. No test logs into ICICI, changes funds, places an order, or connects to a live feed.

## Design and evidence

- [Project brief](docs/PROJECT_BRIEF.md)
- [Architecture and public API](docs/ARCHITECTURE.md)
- [Complete API coverage matrix](docs/API_COVERAGE.md)
- [Official-document audit and ambiguities](docs/DOCUMENTATION_AUDIT.md)
- [Test strategy and verification status](docs/TEST_STRATEGY.md)
- [Implementation and release qualification plan](docs/IMPLEMENTATION_PLAN.md)
- [Security and trading-safety requirements](docs/SECURITY.md)

The contract corpus was reviewed on 2026-08-29 from:

- [Official Breeze API reference](https://api.icicidirect.com/breezeapi/documents/index.html), captured 2026-08-29 and reverified unchanged 2026-08-30 at digest `943a65f477efb1ad594efaed9b239066618f023ea4ab346a34841a90a29ec47e`.
- [Official Breeze Python SDK](https://github.com/Idirect-Tech/Breeze-Python-SDK) version 1.0.68 at commit `4125106b48932ff99b45d593749dcec21c552558`.
- Documentation-linked and SDK-current security-master archives, inspected for file inventory and schema evidence.

The official prose and maintained SDK disagree in several places. The selected behavior and remaining live unknowns are recorded in [the documentation audit](docs/DOCUMENTATION_AUDIT.md), not silently guessed.

## Release boundary

The local implementation can be qualified without broadening authority. A public release still requires explicit decisions on crate name, license, version/semver policy, dependency policy, and publication. Production compatibility additionally requires user-authorized, sanitized live read-only checks. Live mutations are never an automated release gate.
