# Examples

These examples are idiomatic Rust counterparts to the examples in the official
[Breeze Python SDK](https://github.com/Idirect-Tech/Breeze-Python-SDK) README.
The comparison was made against version 1.0.68 at commit
`4125106b48932ff99b45d593749dcec21c552558`, the same revision pinned by this
crate's contract corpus.

They mirror ICICI's documented API workflows, not Python implementation
details. In particular, request builders use typed instruments, decimal money,
positive quantities, and explicit variants instead of loosely validated string
dictionaries.

## Safety boundary

- Authentication and read examples make live ICICI requests when credentials
  are supplied.
- The streaming example opens a live Socket.IO connection when run.
- `mutation_requests` only constructs and validates request values. It never
  authenticates, changes funds, places or modifies an order, squares off a
  position, or changes a GTT order.
- No example reads `.envrc.local` or any other file. Credentials must already be
  present in the process environment.

The examples are usage aids, not live-compatibility evidence. Use a non-trading
account context where possible and stay within ICICI's documented rate limits.

## Authentication variables

Every live API call needs:

```console
export BREEZE_APP_KEY='...'
export BREEZE_SECRET_KEY='...'
```

For live examples other than `authenticate`, supply one session input:

- `BREEZE_SESSION_TOKEN`: restores a CustomerDetails session token; or
- `BREEZE_API_SESSION`: performs the documented CustomerDetails exchange first.

When both are present, the shared example helper uses `BREEZE_SESSION_TOKEN`.
Secret values are never printed. To perform the browser-login exchange itself:

```console
cargo run --example authenticate
```

With only `BREEZE_APP_KEY` set, `authenticate` prints the ICICI login URL and
exits without making an API call. After browser login, export
`BREEZE_SECRET_KEY` and `BREEZE_API_SESSION`, then run it again to perform
CustomerDetails. It does not run a callback server or persist the returned
session token. This target deliberately ignores `BREEZE_SESSION_TOKEN` because
its purpose is to demonstrate the API-session exchange.

## Instrument variables

Market examples construct one typed instrument from these variables:

| Variable | Values | Required for |
|---|---|---|
| `BREEZE_PRODUCT` | `cash`, `futures`, `options` | Every instrument |
| `BREEZE_EXCHANGE` | `NSE`, `BSE`, `NFO`, `BFO`, `NDX`, `MCX` | Every instrument |
| `BREEZE_STOCK_CODE` | ICICI stock code | Every instrument |
| `BREEZE_EXPIRY` | `YYYY-MM-DD` | Futures and options |
| `BREEZE_RIGHT` | `call` or `put` | Options |
| `BREEZE_STRIKE` | Exact decimal text | Options |

The SDK rejects invalid combinations before network I/O. For example, a cash
instrument cannot carry expiry/strike fields and an option-chain request cannot
use NSE cash identity.

## Historical data as CSV

The Python README has separate v1/v2 examples for equity, futures, and options.
The Rust example uses the instrument variables above, so one compile-checked
program covers all six combinations.

Set:

- `BREEZE_HISTORICAL_API` to `v1` or `v2`;
- `BREEZE_INTERVAL` to `minute`, `1minute`, `day`, or `1day`;
- `BREEZE_FROM` and `BREEZE_TO` to RFC 3339 timestamps.

Then write the normalized bars to a CSV file:

```console
cargo run --quiet --example historical_data > historical.csv
```

The output columns include the complete derivative identity, OHLCV, open
interest, and the optional v2 count. Authentication and validation errors go to
the process error stream through Rust's normal error reporting rather than
being mixed into the CSV.

## Quotes and option chains

```console
cargo run --example market_data -- quotes
cargo run --example market_data -- option-chain
```

`option-chain` requires `BREEZE_PRODUCT=options` and uses the complete typed
option identity. This is stricter than the Python call and prevents an
incomplete/non-option instrument from reaching ICICI.

## Account and portfolio reads

```console
cargo run --example account_portfolio -- funds
cargo run --example account_portfolio -- demat
cargo run --example account_portfolio -- margin
cargo run --example account_portfolio -- holdings
cargo run --example account_portfolio -- positions
```

`margin` and `holdings` also require `BREEZE_EXCHANGE`. Each invocation makes
only the selected read call.

## Orders, trades, GTT book, and preview

```console
cargo run --example orders_and_trades -- order-list
cargo run --example orders_and_trades -- order-detail
cargo run --example orders_and_trades -- trade-list
cargo run --example orders_and_trades -- trade-detail
cargo run --example orders_and_trades -- gtt-orders
cargo run --example orders_and_trades -- preview
```

List operations require `BREEZE_FROM` and `BREEZE_TO`. Order/trade detail also
requires `BREEZE_ORDER_ID`. Preview requires the instrument variables plus
`BREEZE_ACTION`, `BREEZE_QUANTITY`, and `BREEZE_PRICE`; it calculates charges
without placing the order.

## Risk calculators

The margin calculator requires an instrument plus `BREEZE_ACTION`,
`BREEZE_QUANTITY`, and `BREEZE_PRICE`:

```console
cargo run --example risk_calculators -- margin
```

The limit-price calculator additionally requires
`BREEZE_STOP_LOSS_TRIGGER`, `BREEZE_SOURCE_FLAG` (`P` or `O`),
`BREEZE_LIMIT_RATE`, `BREEZE_ORDER_ID`, `BREEZE_AVAILABLE_QUANTITY`, and
`BREEZE_FRESH_ORDER_LIMIT`:

```console
cargo run --example risk_calculators -- limit-price
```

Unlike the Python example's empty strings, version 0.0.2 requires a non-empty
order reference and positive available quantity. This known modeling limitation
is not hidden by substituting invented values.

## Streaming

Build with the `streaming` feature:

```console
cargo run --features streaming --example streaming -- quote
cargo run --features streaming --example streaming -- depth
cargo run --features streaming --example streaming -- candles
cargo run --features streaming --example streaming -- orders
cargo run --features streaming --example streaming -- one-click-fno
cargo run --features streaming --example streaming -- one-click-equity
```

Quote/depth/candle operations require `BREEZE_SCRIPT_CODE`. Candles also require
`BREEZE_CANDLE_INTERVAL` (`1SEC`, `1MIN`, `5MIN`, or `30MIN`). The example reads
10 events by default, waits at most 30 seconds for each event, then unsubscribes
and shuts down. Override those bounds with `BREEZE_STREAM_EVENTS` and
`BREEZE_STREAM_TIMEOUT_SECONDS`.

Multiple-token Python subscriptions map to repeated typed `subscribe` calls on
the same handle. The handle deduplicates subscriptions and enforces its local
2,000-subscription cap.

## Mutation request construction

The official Python README directly calls funds, order, square-off, and GTT
mutation methods. The Rust example deliberately stops before I/O:

```console
cargo run --example mutation_requests
```

It demonstrates validated requests for set-funds, place/modify/cancel,
square-off, and single/cover-OCO GTT place/modify/cancel. Applications must add
their own confirmation, static-IP compliance, reconciliation, and explicit call
to the `trading()` or `set_funds()` facade. The repository never automates that
step.

## Python example parity

| Official Python README calls | Rust counterpart |
|---|---|
| `generate_session`, `get_customer_details` | `authenticate`; shared authentication used by every live example |
| `get_demat_holdings`, `get_funds`, `get_margin` | `account_portfolio` |
| `get_portfolio_holdings`, `get_portfolio_positions` | `account_portfolio` |
| `get_historical_data`, `get_historical_data_v2` for cash/futures/options | `historical_data` |
| `get_quotes`, `get_option_chain_quotes` | `market_data` |
| `get_order_list`, `get_order_detail`, `get_trade_list`, `get_trade_detail` | `orders_and_trades` |
| `gtt_order_book`, `preview_order` | `orders_and_trades` |
| `margin_calculator`, `limit_calculator` | `risk_calculators` |
| `ws_connect`, `subscribe_feeds`, `unsubscribe_feeds`, order notifications, One Click | `streaming` |
| `set_funds`, place/modify/cancel, square-off, GTT mutations | offline-only `mutation_requests` construction |

The Python-only `add_margin`, `get_names`, silent aggressive-limit conversion,
BTST extras, and prohibited Margin/Option Plus mutation forms are not replicated.
They are outside the 27 operations on the reviewed main ICICI API page or
conflict with the current regulatory/safety model. See
[`docs/API_COVERAGE.md`](../docs/API_COVERAGE.md) and
[`docs/DOCUMENTATION_AUDIT.md`](../docs/DOCUMENTATION_AUDIT.md).

## Compile every example

```console
cargo check --all-features --examples
cargo clippy --all-features --examples -- -D warnings
```
