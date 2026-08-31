# Official Documentation Audit

## Evidence reviewed

| Source | Observed version | How it is used |
|---|---|---|
| [Breeze API reference](https://api.icicidirect.com/breezeapi/documents/index.html) | Retrieved 2026-08-29 and reverified unchanged 2026-08-30; SHA-256 `943a65f477efb1ad594efaed9b239066618f023ea4ab346a34841a90a29ec47e` | Primary inventory of documented operations, fields, samples, streams, and regulatory text |
| [Breeze Python SDK](https://github.com/Idirect-Tech/Breeze-Python-SDK) | 1.0.68; commit `4125106b48932ff99b45d593749dcec21c552558` | Maintained official cross-check when the page contradicts itself; not authority to add undocumented operations |
| Documentation-linked `NewSecurityMaster/SecurityMaster.zip` | Last-Modified 2026-08-28; five files; inspected 2026-08-29 | Schema evidence for the link on the page |
| SDK-current `MotherAppMaster/SecurityMaster.zip` | Last-Modified 2026-08-28; seven files; inspected 2026-08-29 | Current official-SDK schema/file-set cross-check |

The HTML snapshot and archives were temporary research inputs and are not vendored. Tests contain only small, synthetic, normalized fixtures derived from their documented shapes.

## Precedence rules

1. Current regulatory restrictions and safety notices override convenience behavior.
2. A consistent endpoint table plus matching examples defines the documented wire contract.
3. When the page contradicts itself, the maintained official SDK can resolve spelling, casing, omission, or host details.
4. A sanitized live read-only capture may prove current behavior, but it does not authorize a mutation or erase a documented restriction.
5. If a conflict could change what is traded, how often it is traded, or whether a mutation is duplicated, fail closed and require explicit upstream evidence.

## Material inconsistencies and selected behavior

| Area | Official evidence conflict | Selected Rust contract |
|---|---|---|
| Request encoding | Introduction says all inputs are form-encoded; every maintained example signs compact JSON and sends `application/json`. | Send exact compact JSON bytes for v1, including GET/DELETE bodies. Never form-encode v1. |
| Authentication name | Page calls the flow OAuth 2.0, but documents a browser redirect containing `API_Session`, then a custom CustomerDetails exchange. | Describe the observed custom session exchange; do not claim standards-compliant OAuth token semantics. |
| Required secrets | Page says all requests contain App Key, Secret Key, Session Token, Checksum, Timestamp; the secret is actually signature input and must not be sent. | Secret is local-only. CustomerDetails is unsigned; normal v1 uses the four documented headers plus content type. |
| Timestamp | Header text says “0 milliseconds” and accepts only a 60-second skew. | Always emit `.000Z`; use an injected UTC clock; no local timezone; retry with a fresh timestamp. |
| GET bodies | REST GET examples use JSON request bodies, which are unusual and may be dropped by intermediaries. | Preserve GET bodies exactly and test transport bytes. No query conversion except historical v2. |
| Exchange support | Introduction says BSE and MCX unavailable; the response sample, security masters, historical-v2 sample, and maintained SDK include BSE/BFO/MCX/NDX. | Narrow request enums per endpoint. Historical v2 and stream/instrument types support evidenced exchanges; v1 operations do not gain unsupported exchanges by inference. |
| Option-chain path | Table uses `/OptionChain`; page URLs and maintained SDK use `optionchain`. | Lowercase `/optionchain`. Keep a drift test so a server change is visible. |
| Historical interval | Table lists `1minute` and `1day`; v1 sample sends `day`; Python SDK maps `1minute -> minute` and `1day -> day`. | Public enum uses friendly/documented values; v1 wire adapter maps to `minute`/`day`, v2 sends its own documented values. |
| Historical v2 auth | v2 uses query parameters and only `X-SessionToken` plus `X-apikey`/`apikey`, unlike v1 signing. | Separate v2 request path and signer policy. Header matching is case-insensitive, emitted spelling follows the maintained SDK unless a live fixture disproves it. |
| Historical v2 parameter | Page table says `exchange_code`; examples and SDK send `exch_code`. | Send `exch_code`. |
| Conditional derivative fields | Several tables mark expiry/right/strike mandatory even while notes make them optional for cash/BTST. | `Instrument` variants control fields; cash never asks for meaningless derivative inputs. |
| Portfolio holdings filters | Table marks dates and stock mandatory, marks `portfolio_type` optional without defining its vocabulary; official SDK requires only exchange and omits absent filters. | Require exchange; make date range, stock, and an opaque bounded `PortfolioType` optional and omit each when absent. |
| Trade-list dates | Table says dates mandatory; maintained SDK makes them optional. | Require a date range in the typed request for the documented, bounded behavior. The SDK-only unbounded form is not exposed. |
| Option-chain filters | Table appears to require all fields; official SDK requires at least two of expiry/right/strike for options. | Builder enforces the maintained two-of-three rule and NFO/BFO derivative exchanges. |
| Margin calculator | Parameter table lists one position's fields but omits the actual `list_of_positions` wrapper and `exchange_code`. | Model the wrapper shown by all language examples and the maintained SDK. |
| Limit calculator | Table omits `underlying`, `order_flow`, `source_flag`, and `limit_rate`; examples/SDK include them. | Use the maintained complete payload and retain an audit comment for each extra field. |
| Place-order fields | Page table includes derivative fields unconditionally and calls stoploss a JSON number in one place; examples and SDK omit absent fields and mostly send numeric strings. | Typed instrument plus order builder; serialize decimal fields as strings; omit absent optionals. |
| Modify-order fields | The table repeats expiry/right/strike, but examples and the maintained SDK modify by order id/exchange plus only changed order fields. | Do not resend immutable instrument identity. The builder exposes quantity, price, limit/stop-loss type, trigger, validity, and disclosed quantity and requires at least one real change. |
| `validity_date` | Tables expose it; notes say the broker excludes it from place, modify, and square-off processing. | Exclude it from mutation builders. Decode a returned validity date on order responses, but do not offer a no-effect request option. |
| Market orders | Regulatory page says market orders are not permitted; Python SDK 1.0.68 silently computes and sends an aggressive limit order for `market`. | No `Market` order variant and no silent conversion. A future explicit `AggressiveLimit` helper requires independent tests and naming. |
| Square-off legacy optionals | The table names source/protection/settlement/margin/cover/alias/trade-password fields without enough semantics, while examples send empty values. | The typed builder exposes meaningful limit/stop-loss, validity, open quantity, and disclosed quantity. Undefined legacy fields remain empty/zero and no trade-password/raw mutation setter is exposed. |
| Mutation rate | General API limit is 100/min and 5,000/day; regulatory section separately caps combined order mutations at 10/sec. | Apply both gates. Never interpret one as replacing the other. |
| Margin/Option Plus orders | Regulatory section prohibits place/modify/cancel of these order products; request enums elsewhere are broader. | Do not expose prohibited variants in mutation builders without new regulatory evidence. |
| Preview host | Table and Python/JavaScript use production; Java/C# snippets point to a UAT host. | Use production endpoint configuration. Never ship UAT as a fallback. |
| Preview response | Both samples are malformed JSON; F&O begins with `{}` and both close an object with `)`. | Store corrected normalized fixtures, record the repairs, and require live read-only confirmation before freezing model semantics. |
| GTT place/modify shape | GTT tables repeat flat placement fields; the cover-OCO example and maintained SDK use `order_details` arrays, while the official SDK supplies a distinct single-leg payload. | Typed `single` and `cover_oco` constructors serialize one leg or a target/stop-loss pair. Modify sends only exchange, id, type, and legs. |
| GTT vocabulary | Page documents `single` and `cover_oco`, one modify sample contains `ocox`, responses use title-cased `Cover OCO`, and the maintained SDK additionally accepts plain `oco`. | Serialize only documented `single` and `cover_oco`; treat `ocox` as the sample's typo and leave SDK-only `oco` in the parity backlog. |
| Stream protocol | prose says WebSocket, examples use Python Socket.IO, event names, and join/leave messages. | Implement Socket.IO over WebSocket transport; do not use a raw WebSocket client directly. |
| Stream field names | Tick table/sample/parser disagree (`bQty`/`boty`, `totalSellQt`/`totalSellQ`/`totalSello`), and order sample has broken quotes/keys. | Public models use semantic snake_case fields; decoders recognize documented positional layouts and JSON aliases; unknown data is preserved. |
| One Click Equity frame | The page shows post-parse dictionaries, while the maintained SDK receives and maps a raw 19-position Socket.IO list. | Decode the strict 19-position raw list before exposing semantic fields; do not treat the Python SDK's normalized callback dictionary as the wire frame. |
| OHLCV field order | The page and maintained SDK define the positional price fields as low, high, open, close; samples with equal prices can conceal an index swap. | Map low/high/open/close in that exact order for equity, option, and future frames and prove it with distinct-price fixtures. |
| Stream host examples | Some non-Python snippets call `breezeapi.icicidirect.com`, while request tables and Socket.IO examples use `livefeeds` or `livestream`. | Use family-specific hosts from the full Socket.IO examples and maintained SDK. |
| Security-master URL | Page links `NewSecurityMaster` (five files); current SDK uses `MotherAppMaster` (seven files including MCX and MF). | The core recognizes all observed filenames and parses caller-provided readers by header/file identity. No source preference or downloader is shipped. |
| Security-master schema | The archive contains multiple incompatible CSV layouts and mixed spacing/casing. | Header-driven per-file adapters, unknown-column tolerance, and synthetic tests for all observed schemas. |
| Error list | Page lists only 200/400/401/403/404/408 and misspells “Not Found”; real services can return 429 and 5xx. | Cover all HTTP categories, Breeze application errors, invalid JSON, body limits, and unknown statuses. |

## Normalization log for fixtures

The official response blocks often use Python literals (`'single quotes'`, `None`) under a JSON label. The fixture corpus makes only syntax-level repairs:

- single-quoted object keys and strings become JSON strings;
- `None` becomes `null`;
- malformed preview-order braces/parentheses are repaired to the apparent envelope;
- obvious sample credentials/account identifiers are replaced with synthetic values;
- no undocumented semantic value is invented to fill an absent field.

The One Click Equity wire fixture is a synthetic raw 19-position array following the maintained SDK's pre-parse mapping. The adjacent dictionaries shown by the page are callback-output examples and inform field semantics, but they are not treated as Socket.IO wire frames.

Every normalized response retains the complete set of fields shown by its source sample. Where a typo could be a real wire key, aliases and raw-frame tests preserve the ambiguity instead of deleting it.

## Unknowns that block a 1.0 claim, not implementation work

- Exact current Socket.IO Engine.IO protocol version and server heartbeat limits.
- Whether v1 option-chain path casing is accepted both ways in production.
- Current error-envelope shapes for 429 and 5xx.
- Whether the 5,000/day reset is calendar-day, rolling, account-level, or API-key-level.
- Maximum response sizes and list pagination/truncation behavior; no pagination is documented.
- Current production response shapes for preview order and every empty-result case.
- Formal server-side idempotency support for mutations; none is documented.

These are resolved through ICICI support or opt-in, sanitized, read-only compatibility probes. They must not be guessed into a stable public contract.
