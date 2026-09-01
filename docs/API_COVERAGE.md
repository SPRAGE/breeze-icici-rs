# API Coverage Matrix

## Scope and counting

The reviewed reference page contains 27 REST operation sections, five streaming families, and global contracts for login, signing, errors, instruments, limits, and regulatory restrictions. The table below accounts for every one of them.

“Contract green” means one selected canonical request and normalized response shape pass the hermetic black-box suite. It does **not** mean every documented variant is implemented or that the behavior has been live-verified against the current broker service. See `KNOWN_LIMITATIONS.md` for the gaps known at version `0.0.1`.

## Global contracts

| Documentation section | Implemented Rust surface | Contract proof |
|---|---|---|
| Introduction | `RateLimitPolicy::documented_defaults`, `EndpointSet` | Fixture manifest asserts call limits, hosts, and source revision |
| Regulatory Changes | closed mutation request types; no `Market` order kind; mutation rate class | Validation, rate-limit, and no-mutation-retry tests |
| Checksum Computation & Login | `login_url`, `ApiSession`, `SessionToken`, v1 signer | Exact digest/header/body vectors and malformed-session tests |
| Request Headers | private signer plus feature-gated prepared-request view | Header presence, timestamp format, redaction, and exact-body tests |
| Errors | stable `Error` categories | HTTP/application/malformed/unknown/timeout fixtures |
| Instruments | `SecurityMaster`, `InstrumentKey`, `ScriptCode` | Per-schema CSV fixtures, lookup, duplicate, and malformed-row tests |

## REST operations

| # | Documentation section | Method and wire endpoint | Implemented Rust request/service | Fixture key | Status |
|---:|---|---|---|---|---|
| 1 | CustomerDetails / CustomerDetails | `GET /customerdetails` | pending client `.authenticate(ApiSession)` | `auth.customer_details` | Implemented, contract green |
| 2 | DematHoldings / GetDematHoldings | `GET /dematholdings` | `account().demat_holdings()` | `account.demat_holdings` | Implemented, contract green |
| 3 | Funds / GetFunds | `GET /funds` | `account().funds()` | `account.get_funds` | Implemented, contract green |
| 4 | Funds / SetFunds | `POST /funds` | `account().set_funds(SetFundsRequest)` | `account.set_funds` | Implemented, contract green |
| 5 | HistoricalCharts / GetHistoricalChartsList | `GET /historicalcharts` with signed JSON body | `market_data().historical_v1(HistoricalV1Request)` | `market.historical_v1` | Implemented, contract green |
| 6 | Margin Calculator / margin calculator | `POST /margincalculator` | `risk().calculate_margin(MarginCalculationRequest)` | `risk.margin_calculator` | Implemented, contract green |
| 7 | Margin / GetMargins | `GET /margin` | `account().margin(Exchange)` | `account.get_margin` | Implemented, contract green |
| 8 | Order / OrderPlacement | `POST /order` | `trading().place(PlaceOrderRequest)` | `orders.place` | Implemented, contract green |
| 9 | Order / GetOrderDetail | `GET /order` | `orders().detail(OrderDetailRequest)` | `orders.detail` | Implemented, contract green |
| 10 | Order / GetOrderList | `GET /order` | `orders().list(OrderListRequest)` | `orders.list` | Implemented, contract green |
| 11 | Order / OrderCancellation | `DELETE /order` | `trading().cancel(CancelOrderRequest)` | `orders.cancel` | Implemented, contract green |
| 12 | Order / OrderModification | `PUT /order` | `trading().modify(ModifyOrderRequest)` | `orders.modify` | Implemented, contract green |
| 13 | Breeze Limit price calculation / limit calculator | `POST /fnolmtpriceandqtycal` | `risk().limit_price(LimitPriceRequest)` | `risk.limit_price` | Implemented, contract green |
| 14 | PortfolioHoldings / GetPortfolioHoldings | `GET /portfolioholdings` | `portfolio().holdings(HoldingsRequest)`, including optional `PortfolioType` | `portfolio.holdings` | Implemented, contract green |
| 15 | PortfolioPositions / GetPortfolioPositions | `GET /portfoliopositions` | `portfolio().positions()` | `portfolio.positions` | Implemented, contract green |
| 16 | Quotes / GetQuotes | `GET /quotes` | `market_data().quotes(QuoteRequest)` | `market.quotes` | Implemented, contract green |
| 17 | SquareOff / SquareOff | `POST /squareoff` | `trading().square_off(SquareOffRequest)` | `orders.square_off` | Implemented, contract green |
| 18 | Trades / GetTradeList | `GET /trades` | `trades().list(TradeListRequest)` | `trades.list` | Implemented, contract green |
| 19 | Trades / GetTradeDetail | `GET /trades` | `trades().detail(TradeDetailRequest)` | `trades.detail` | Implemented, contract green |
| 20 | OptionChain / GetOptionChain | `GET /optionchain` | `market_data().option_chain(OptionChainRequest)` | `market.option_chain` | Implemented, contract green |
| 21 | Preview Order / GetBrokeragecharges - Equity | `GET /preview_order` | `orders().preview(PreviewOrderRequest)` with equity instrument | `orders.preview_equity` | Implemented, contract green |
| 22 | Preview Order / GetBrokeragecharges - Fno | `GET /preview_order` | `orders().preview(PreviewOrderRequest)` with derivative instrument | `orders.preview_fno` | Implemented, contract green |
| 23 | HistoricalChartsv2 / GetHistoricalCharts | v2 `GET /api/v2/historicalcharts` with query | `market_data().historical(HistoricalV2Request)` | `market.historical_v2` | Implemented, contract green |
| 24 | GTTOrder / GTTOrderPlacement | `POST /gttorder` | `trading().place_gtt(GttOrderRequest)` with documented single-leg/cover-OCO constructors | `gtt.place` | Implemented, contract green |
| 25 | GTTOrder / GTTOrderBook | `GET /gttorder` | `orders().gtt_orders(GttOrderListRequest)` | `gtt.list` | Implemented, contract green |
| 26 | GTTOrder / GTTCancelOrder | `DELETE /gttorder` | `trading().cancel_gtt(CancelGttOrderRequest)` | `gtt.cancel` | Implemented, contract green |
| 27 | GTTOrder / GTTModifyOrder | `PUT /gttorder` | `trading().modify_gtt(ModifyGttOrderRequest)` for documented single-leg/cover-OCO types | `gtt.modify` | Implemented, contract green |

The option-chain path is deliberately lowercase in the selected contract. The documentation table shows `/OptionChain`, while its own links and ICICI's maintained SDK also use lowercase. See the audit for this and other resolutions.

## Example coverage

The compile-checked [example suite](../examples/README.md) maps the official
Python SDK README examples onto the typed Rust services above. Multiple Python
snippets that differ only by instrument strings share one Rust example built on
`Instrument`, so equity/future/option identity is validated rather than copied
between samples. Mutation examples construct requests only and never dispatch
them.

## Streaming families

| Documentation section | Protocol contract | Implemented Rust surface | Status |
|---|---|---|---|
| Order Notifications | Socket.IO `order` event on `livefeeds.icicidirect.com`; cash and derivative layouts | `streaming()?.connect(StreamKind::Orders)` and `decode_order_notification` | Implemented; hermetic green; live unverified |
| Tick Data Stream | join/leave script codes; Socket.IO `stock` event on `livestream.icicidirect.com`; quotes/depth/commodity layouts | `streaming()?.connect(StreamKind::MarketData)` and `decode_tick` | Implemented; hermetic green; live unverified |
| One Click F&O Stream | join `["one_click_fno"]`; `stock` event | `streaming()?.connect(StreamKind::OneClickFno)` | Implemented; hermetic green; live unverified |
| One Click Equity Stream | join `["i_click_2_gain"]`; `stock` event; raw 19-position list | `streaming()?.connect(StreamKind::OneClickEquity)` | Implemented; raw-frame contract green; live unverified |
| Candle Stream | `/ohlcvstream`; interval event; equity/future/option CSV layouts with low/high/open/close ordering | `streaming()?.connect(StreamKind::Candles)` and `decode_candle` | Implemented; distinct-price contracts green; live unverified |

Streaming contracts also cover strict session-token decoding, a 2,000-script local cap, subscription deduplication, malformed frames, unknown variants, bounded queues, reconnect replay, unsubscribe, and shutdown.

## Request-field coverage

The implemented request builders and exact-wire tests assign documented fields to one of four categories:

- required and always serialized;
- conditionally required by an instrument/request variant;
- optional and omitted when absent;
- documented but broker-ignored and intentionally excluded from the normal typed API.

The canonical operation bytes live in `tests/fixtures/wire_contracts.json`; additional variant tests cover derivative quote/historical-v1 contract identity, fail-closed option-chain conversion, stop-loss orders, optional `portfolio_type`, and single-leg GTT place/modify shapes. The typed API does not expose `validity_date` on place/modify/square-off builders because the docs explicitly state that it has no execution effect.

## Response-field coverage

`tests/fixtures/rest_success.json` normalizes every official response example into valid JSON and retains every field shown by that example. Contract tests require:

- string-or-number decimal compatibility where the examples vary;
- explicit null and empty-string handling;
- unknown-field tolerance;
- preservation of unknown enum strings;
- correct `Success` object versus list shape;
- application-error handling even when HTTP and Breeze status disagree.

Normalized fixtures are evidence of documentation shape, not proof that current production always returns that shape. Sanitized live read-only captures are a separate release gate.

## Official-SDK parity backlog, not part of this page's 27 operations

The maintained Python SDK exposes behavior not fully specified on the reviewed page, including `add_margin`, name lookup helpers, additional order fields, more exchange variants, and aggressive-limit conversion. These are recorded as a parity backlog only. They must not enter the Rust 1.0 API without endpoint evidence, safety review, and new contract fixtures.
