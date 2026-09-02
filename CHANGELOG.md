# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

## [0.0.1] - 2026-08-30

- Added compile-checked Rust examples mapped to the official Python SDK example
  corpus, with live reads bounded by explicit inputs and mutation examples kept
  offline-only.
- Prepared the experimental source release for crates.io distribution under
  the Apache-2.0 license, without making a production-readiness claim.

- Added an async, typed REST client with pending/authenticated typestates.
- Added exact compact-JSON signing, including signed `GET` and `DELETE` bodies.
- Added request and response foundations for the 27 reviewed REST operations.
- Added feature-gated Socket.IO stream codecs and lifecycle support.
- Added hermetic wire, response, transport, validation, compile-fail, and stream tests.
- Added an explicit production-readiness and AI-generated-code warning.

[0.0.1]: https://github.com/SPRAGE/breeze-icici-rs/releases/tag/v0.0.1
