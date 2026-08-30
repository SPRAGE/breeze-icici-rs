use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use breeze_icici::EndpointRequest;
use breeze_icici::auth::{AppKey, Credentials, SecretKey, SessionToken};
use breeze_icici::client::{Authenticated, BreezeClient, EndpointSet};
use breeze_icici::domain::{
    DateRange, DerivativeExchange, Exchange, Instrument, Money, OptionRight, Quantity, StockCode,
};
use breeze_icici::testing::{FixedClock, PreparedRequest};
use chrono::{DateTime, NaiveDate, Utc};
use serde::de::DeserializeOwned;
use serde_json::Value;
use url::Url;

pub const APP_KEY: &str = "app-key-test";
pub const SECRET_KEY: &str = "secret-key-test";
pub const SESSION_TOKEN: &str = "dXNlci10ZXN0OnNlc3Npb24tdGVzdA==";
pub const API_SESSION: &str = "api-session-test";
pub const FIXED_TIMESTAMP: &str = "2025-02-05T10:11:12.000Z";

pub fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

pub fn fixture<T: DeserializeOwned>(name: &str) -> T {
    let path = fixture_path(name);
    let bytes = fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    serde_json::from_slice(&bytes)
        .unwrap_or_else(|error| panic!("parse {}: {error}", path.display()))
}

pub fn response_fixture(id: &str) -> Vec<u8> {
    let responses: BTreeMap<String, Value> = fixture("rest_success.json");
    serde_json::to_vec(&responses[id]).expect("serialize normalized response fixture")
}

pub fn stream_fixture(id: &str) -> Value {
    let frames: BTreeMap<String, Value> = fixture("stream_frames.json");
    frames[id].clone()
}

pub fn credentials() -> Credentials {
    Credentials::new(
        AppKey::new(APP_KEY).expect("synthetic app key"),
        SecretKey::new(SECRET_KEY).expect("synthetic secret key"),
    )
}

pub fn fixed_time() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(FIXED_TIMESTAMP)
        .expect("fixed RFC3339 timestamp")
        .with_timezone(&Utc)
}

pub fn production_shape_test_endpoints(origin: &str) -> EndpointSet {
    let root = Url::parse(origin).expect("test origin URL");
    EndpointSet::builder()
        .rest_v1(root.join("breezeapi/api/v1/").expect("v1 URL"))
        .rest_v2(root.join("api/v2/").expect("v2 URL"))
        .live_feeds(root.join("livefeeds/").expect("live-feeds URL"))
        .live_stream(root.join("livestream/").expect("live-stream URL"))
        .ohlcv(root.join("ohlcv/").expect("OHLCV URL"))
        .allow_insecure_loopback_for_tests()
        .build()
        .expect("synthetic endpoints")
}

pub fn client() -> BreezeClient<Authenticated> {
    client_with_endpoints(EndpointSet::production())
}

pub fn client_with_endpoints(endpoints: EndpointSet) -> BreezeClient<Authenticated> {
    BreezeClient::builder(credentials())
        .session_token(SessionToken::new(SESSION_TOKEN).expect("synthetic session token"))
        .clock(Arc::new(FixedClock::new(fixed_time())))
        .endpoints(endpoints)
        .build()
        .expect("synthetic authenticated client")
}

pub fn prepared<R: EndpointRequest>(request: R) -> PreparedRequest {
    breeze_icici::testing::prepare(&client(), request).expect("request must prepare")
}

pub fn date(value: &str) -> NaiveDate {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").expect("fixture date")
}

pub fn time(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .expect("fixture date-time")
        .with_timezone(&Utc)
}

pub fn range(from: &str, to: &str) -> DateRange {
    DateRange::new(time(from), time(to)).expect("valid fixture range")
}

pub fn money(value: &str) -> Money {
    Money::from_str(value).expect("valid fixture money")
}

pub fn quantity(value: u64) -> Quantity {
    Quantity::new(value).expect("positive fixture quantity")
}

pub fn stock(value: &str) -> StockCode {
    StockCode::new(value).expect("valid fixture stock code")
}

pub fn equity() -> Instrument {
    Instrument::equity(Exchange::Nse, stock("ITC")).expect("valid equity")
}

pub fn future() -> Instrument {
    Instrument::future(DerivativeExchange::Nfo, stock("NIFTY"), date("2025-02-27"))
        .expect("valid future")
}

pub fn option() -> Instrument {
    Instrument::option(
        DerivativeExchange::Nfo,
        stock("NIFTY"),
        date("2025-02-27"),
        OptionRight::Call,
        money("24000"),
    )
    .expect("valid option")
}
