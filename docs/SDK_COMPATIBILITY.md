# Official SDK compatibility

Existing APIs target Breeze Python SDK 1.0.68, commit
`4125106b48932ff99b45d593749dcec21c552558`. For overlapping requests, its
defaults, accepted inputs, and omissions take precedence over conflicting
reference-page examples. Current trading restrictions and explicit mutation
safety boundaries still apply.

## Evidence and reproduction

`tests/fixtures/sdk_wire_contracts.json` contains 44 synthetic captures from the
SDK's actual request methods: 27 canonical REST operations and 17 variant or
rejection cases. `tests/contract/sdk_compat.rs` compares method, endpoint, JSON
payload, actual URL, selected authentication/content headers, and v2 queries.
Object ordering can differ; separate exact-wire tests
prove each checksum matches its own transmitted compact body bytes.

`scripts/capture_sdk_contract.py` verifies normalized SDK source/config hashes
and loads only their class/config definitions. All request I/O is replaced by
capture functions. It never imports the SDK module, runs its downloads/log
setup, uses live credentials, or contacts ICICI. Its synthetic order-detail stub
records the SDK's read-before-modify call. A separate loopback test proves the
Rust facade performs that read and skips writes for unmodifiable/missing orders.

Canonical arguments come from the Rust wire fixture to compare equivalent
inputs; additional high-value variants have independently authored inputs.
The SDK forwards nested margin positions and GTT legs without validation, so
their inner broker semantics remain reference/local-fixture evidence, not
independent SDK or live evidence.

Normal Cargo tests need neither Python nor an installed SDK. Reproduce captures
using a local copy of the pinned source:

```console
python3 scripts/capture_sdk_contract.py --sdk-dir /path/to/sdk/breeze_connect --check
```

Different source hashes are rejected. Updating the pin requires source review
and intentional fixture regeneration.

## Aligned behavior

- Limit calculations infer a nonempty product. The reference-only blank product
  option is removed. Existing-order fields can remain blank, quantity can be
  zero, and P/M/O source selectors retain their literal values.
- Preview flags default to empty; N/S remain explicit. Read-only stop-loss
  previews may omit their trigger, matching SDK validation.
- Ordinary margin positions omit cover fields. `include_cover_fields()` enables
  neutral N/zero values; setting a cover field also enables this group.
- V2 supports one-second history and JSON content type. V1 rejects one-second
  intervals and BSE/BFO, matching the SDK's v1 vocabulary.
- Futures chains may omit expiry; `TradeListRequest::all(exchange)` omits dates.
- Placement sends SDK routing fields and an empty remark when unset. Typed
  setters support settlement, segment, and lots; validity includes VTC.
  Square-off includes the SDK's empty legacy metadata fields.
- GTT book/cancel/modify accept an explicit exchange, defaulting to NFO.
- `trading().modify()` reads current order state and writes only for ordered,
  requested, or partially executed orders. Lower-level `execute(request)`
  retains direct signed-request semantics without facade workflow checks.

## Scope and limits

This proves request compatibility for supported typed forms of existing APIs,
not complete Python feature parity or a drop-in Python interface. SDK-only
endpoints/helpers, automatic security-master downloads, plain OCO, and arbitrary
legacy mutation setters remain outside this change. Market-to-aggressive-limit
conversion, prohibited Margin/Option Plus products, and MTF placement remain
unavailable. Rust retains positive order quantities, exact decimal responses,
secret redaction, bounded transport behavior, and no automatic write retry.
SDK logging, response dictionaries, and incidental browser User-Agent strings
are not public Rust API contracts.

Rate limits are unchanged. Captured-source and loopback proof do not establish
live account, exchange, Socket.IO, or production readiness.
