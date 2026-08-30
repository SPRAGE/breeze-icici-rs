//! Async, typed client for the ICICI Direct Breeze REST and streaming APIs.
//!
//! The crate preserves Breeze's exact compact-JSON signing contract, including
//! request bodies on `GET` and `DELETE`. Trading mutations are explicit, market
//! orders are not representable, and ambiguous mutations are never retried.
//!
//! # Restoring a session
//!
//! ```no_run
//! use breeze_icici::{AppKey, BreezeClient, Credentials, SecretKey, SessionToken};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let credentials = Credentials::new(
//!     AppKey::new(std::env::var("BREEZE_APP_KEY")?)?,
//!     SecretKey::new(std::env::var("BREEZE_SECRET_KEY")?)?,
//! );
//! let client = BreezeClient::builder(credentials)
//!     .session_token(SessionToken::new(std::env::var("BREEZE_SESSION_TOKEN")?)?)
//!     .build()?;
//! let funds = client.account().funds().await?;
//! println!("unallocated balance: {}", funds.unallocated_balance());
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_debug_implementations)]

pub mod account;
pub mod auth;
pub mod client;
mod clock;
pub mod domain;
pub mod error;
pub mod gtt;
pub mod instruments;
pub mod market;
pub mod orders;
pub mod portfolio;
pub mod rate_limit;
pub mod risk;
mod signing;
pub mod streaming;
pub mod trades;

#[cfg(feature = "test-util")]
pub mod testing;

mod request;
mod services;

pub use auth::{ApiSession, AppKey, Credentials, SecretKey, SessionToken, login_url};
pub use client::{
    AccountService, Authenticated, BreezeClient, BreezeClientBuilder, EndpointSet,
    MarketDataService, OrdersService, Pending, PortfolioService, RetryPolicy, RiskService,
    Timeouts, TradesService, TradingService,
};
pub use error::{Error, TimeoutPhase, ValidationError};
pub use request::EndpointRequest;
