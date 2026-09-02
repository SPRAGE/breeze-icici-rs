# Known limitations

Version `0.0.1` is an experimental source release. Its hermetic tests prove the selected canonical fixtures and local safety behavior; they do not prove every documented variant or live ICICI Direct compatibility.

The following areas require a tests-first hardening pass before production use or a stable release:

- option-chain modeling currently represents options only; the maintained Python SDK's futures form remains unmodeled, while non-option instruments and non-NFO/BFO exchanges are rejected before I/O;
- calculator and GTT models cannot represent every documented empty/zero-valued input and do not encode every exchange/date invariant;
- signed response money values represented as JSON numbers can pass through floating-point deserialization;
- several response fields remain opaque strings or raw JSON values;
- production streaming and the fake lifecycle harness do not yet share one fault-injectable state machine, and no live Socket.IO handshake has been verified;
- mutual-fund security-master rows are recognized but not currently materialized as instrument records;
- a no-default-feature build has no TLS backend and needs a compile-time feature guard;
- public API documentation, hosted MSRV/CI evidence and enforcement, dependency policy, and independent security/API review remain qualification gates.

No automated test or release procedure sends funds, orders, square-offs, or GTT mutations to a live account.
