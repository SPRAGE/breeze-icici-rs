# Known limitations

Version `0.0.3` is an experimental preview. Its hermetic tests prove the selected canonical fixtures and local safety behavior; they do not prove every documented variant or live ICICI Direct compatibility.

The following areas require a tests-first hardening pass before production use or a stable release:

- calculator and GTT models do not yet encode every endpoint-specific exchange/date invariant;
- several response fields remain opaque strings or raw JSON values;
- production streaming and the fake lifecycle harness do not yet share one fault-injectable state machine, and no live Socket.IO handshake has been verified;
- mutual-fund security-master rows are recognized but not currently materialized as instrument records;
- a no-default-feature build has no TLS backend and needs a compile-time feature guard;
- public API documentation, hosted MSRV/CI evidence and enforcement, dependency policy, and independent security/API review remain qualification gates.

No automated test or release procedure sends funds, orders, square-offs, or GTT mutations to a live account.

The 0.0.3 API corrections add futures option chains and cover-OCO, the
documented calculator blanks/zero counts and cover fields, 5/30-minute history,
BTST placement and BTST/MTF previews, exact JSON numeric money decoding, and
candle identity/commodity depth/currency exchange fixes. These have hermetic
regression coverage; they are not a live compatibility qualification.

SDK request compatibility is checked against 44 pinned-source captures for
existing APIs; it is not complete Python feature parity. See `SDK_COMPATIBILITY.md`
for excluded SDK-only features, typed safety restrictions, and proof boundaries.
