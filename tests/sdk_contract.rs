//! Black-box acceptance contract for the `breeze_icici` library.
//!
//! Run it with `--features sdk-contract`. These tests remain fixture-backed and
//! hermetic: they never use a live brokerage account or place an order.

#[path = "contract/auth.rs"]
mod auth;
#[path = "contract/errors.rs"]
mod errors;
#[path = "contract/instruments.rs"]
mod instruments;
#[path = "contract/public_api.rs"]
mod public_api;
#[path = "contract/rate_limit.rs"]
mod rate_limit;
#[path = "contract/responses.rs"]
mod responses;
#[path = "contract/rest_wire.rs"]
mod rest_wire;
#[path = "contract/streaming.rs"]
mod streaming;
#[path = "contract/support.rs"]
mod support;
#[path = "contract/transport.rs"]
mod transport;
#[path = "contract/validation.rs"]
mod validation;
