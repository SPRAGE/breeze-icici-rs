use std::collections::BTreeMap;

use chrono::NaiveDate;
use http::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::EndpointRequest;
use crate::domain::{
    Count, DateRange, DerivativeExchange, Instrument, Interval, Money, OptionRight, StockCode,
    expiry_timestamp,
};
use crate::error::{Error, ValidationError};
use crate::request::{AuthenticationMode, EndpointBase, compact_json};

impl crate::request::sealed::Sealed for HistoricalV1Request {}
impl crate::request::sealed::Sealed for HistoricalV2Request {}
impl crate::request::sealed::Sealed for QuoteRequest {}
impl crate::request::sealed::Sealed for OptionChainRequest {}

#[derive(Clone, Debug, Deserialize)]
pub struct HistoricalBar {
    datetime: String,
    stock_code: StockCode,
    exchange_code: String,
    #[serde(default)]
    product_type: Option<String>,
    #[serde(default)]
    expiry_date: Option<String>,
    #[serde(default)]
    right: Option<String>,
    #[serde(default)]
    strike_price: Option<Money>,
    open: Money,
    high: Money,
    low: Money,
    close: Money,
    volume: Count,
    #[serde(default)]
    open_interest: Option<Money>,
    #[serde(default)]
    count: Option<u64>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

impl HistoricalBar {
    pub fn datetime_raw(&self) -> &str {
        &self.datetime
    }
    pub fn stock_code(&self) -> &StockCode {
        &self.stock_code
    }
    pub fn exchange_code(&self) -> &str {
        &self.exchange_code
    }
    pub fn product_type(&self) -> Option<&str> {
        self.product_type.as_deref()
    }
    pub fn expiry_date_raw(&self) -> Option<&str> {
        self.expiry_date.as_deref()
    }
    pub fn right_raw(&self) -> Option<&str> {
        self.right.as_deref()
    }
    pub fn strike_price(&self) -> Option<&Money> {
        self.strike_price.as_ref()
    }
    pub fn open(&self) -> &Money {
        &self.open
    }
    pub fn high(&self) -> &Money {
        &self.high
    }
    pub fn low(&self) -> &Money {
        &self.low
    }
    pub fn close(&self) -> &Money {
        &self.close
    }
    pub fn volume(&self) -> Count {
        self.volume
    }
    pub fn open_interest(&self) -> Option<&Money> {
        self.open_interest.as_ref()
    }
    pub fn count(&self) -> Option<u64> {
        self.count
    }
    pub fn extra(&self) -> &BTreeMap<String, Value> {
        &self.extra
    }
}

#[derive(Clone, Debug)]
pub struct HistoricalV1Request {
    interval: Interval,
    range: DateRange,
    instrument: Instrument,
}
impl HistoricalV1Request {
    pub fn new(
        interval: Interval,
        range: DateRange,
        instrument: Instrument,
    ) -> Result<Self, ValidationError> {
        Ok(Self {
            interval,
            range,
            instrument,
        })
    }
}
impl EndpointRequest for HistoricalV1Request {
    type Response = Vec<HistoricalBar>;
    fn operation(&self) -> &'static str {
        "historicalcharts"
    }
    fn method(&self) -> Method {
        Method::GET
    }
    fn path(&self) -> &'static str {
        "/historicalcharts"
    }
    fn body(&self) -> Result<Vec<u8>, Error> {
        #[derive(Serialize)]
        struct Body<'a> {
            interval: &'a str,
            from_date: String,
            to_date: String,
            stock_code: &'a str,
            exchange_code: &'a str,
            product_type: &'a str,
        }
        compact_json(&Body {
            interval: self.interval.v1_wire(),
            from_date: self.range.start_wire(),
            to_date: self.range.end_wire(),
            stock_code: self.instrument.stock_code().as_str(),
            exchange_code: self.instrument.exchange_wire(),
            product_type: self.instrument.product().wire(),
        })
    }
}

#[derive(Clone, Debug)]
pub struct HistoricalV2Request {
    interval: Interval,
    range: DateRange,
    instrument: Instrument,
}
impl HistoricalV2Request {
    pub fn new(
        interval: Interval,
        range: DateRange,
        instrument: Instrument,
    ) -> Result<Self, ValidationError> {
        Ok(Self {
            interval,
            range,
            instrument,
        })
    }
}
impl EndpointRequest for HistoricalV2Request {
    type Response = Vec<HistoricalBar>;
    fn operation(&self) -> &'static str {
        "historicalcharts"
    }
    fn method(&self) -> Method {
        Method::GET
    }
    fn path(&self) -> &'static str {
        "/historicalcharts"
    }
    fn endpoint_base(&self) -> EndpointBase {
        EndpointBase::RestV2
    }
    fn authentication(&self) -> AuthenticationMode {
        AuthenticationMode::SessionV2
    }
    fn body(&self) -> Result<Vec<u8>, Error> {
        Ok(Vec::new())
    }
    fn query(&self) -> Vec<(String, String)> {
        let mut values = vec![
            ("interval".into(), self.interval.v2_wire().into()),
            ("from_date".into(), self.range.start_wire()),
            ("to_date".into(), self.range.end_wire()),
            (
                "stock_code".into(),
                self.instrument.stock_code().as_str().into(),
            ),
            ("exch_code".into(), self.instrument.exchange_wire().into()),
            (
                "product_type".into(),
                self.instrument.product().wire().into(),
            ),
        ];
        if let Some(expiry) = self.instrument.expiry() {
            values.push(("expiry_date".into(), expiry.format("%Y-%m-%d").to_string()));
        }
        if let Some(right) = self.instrument.right() {
            values.push(("right".into(), right.wire().into()));
        }
        if let Some(strike) = self.instrument.strike() {
            values.push(("strike_price".into(), strike.to_wire_string()));
        }
        values
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Quote {
    exchange_code: String,
    #[serde(default)]
    product_type: Option<String>,
    stock_code: StockCode,
    #[serde(default)]
    expiry_date: Option<String>,
    #[serde(default)]
    right: Option<String>,
    #[serde(default)]
    strike_price: Option<Money>,
    #[serde(rename = "ltp")]
    last_price: Money,
    #[serde(default)]
    ltt: Option<String>,
    #[serde(default)]
    best_bid_price: Option<Money>,
    #[serde(default)]
    best_bid_quantity: Option<Count>,
    #[serde(default)]
    best_offer_price: Option<Money>,
    #[serde(default)]
    best_offer_quantity: Option<Count>,
    #[serde(default)]
    open: Option<Money>,
    #[serde(default)]
    high: Option<Money>,
    #[serde(default)]
    low: Option<Money>,
    #[serde(default)]
    previous_close: Option<Money>,
    #[serde(default)]
    open_interest: Option<Money>,
    #[serde(alias = "chnge_oi", default)]
    change_in_open_interest: Option<Money>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}
impl Quote {
    pub fn exchange_code(&self) -> &str {
        &self.exchange_code
    }
    pub fn product_type(&self) -> Option<&str> {
        self.product_type.as_deref()
    }
    pub fn last_price(&self) -> &Money {
        &self.last_price
    }
    pub fn expiry_date_raw(&self) -> Option<&str> {
        self.expiry_date.as_deref()
    }
    pub fn right_raw(&self) -> Option<&str> {
        self.right.as_deref()
    }
    pub fn strike_price(&self) -> Option<&Money> {
        self.strike_price.as_ref()
    }
    pub fn last_trade_time_raw(&self) -> Option<&str> {
        self.ltt.as_deref()
    }
    pub fn best_bid_price(&self) -> Option<&Money> {
        self.best_bid_price.as_ref()
    }
    pub fn best_bid_quantity(&self) -> Option<Count> {
        self.best_bid_quantity
    }
    pub fn stock_code(&self) -> &StockCode {
        &self.stock_code
    }
    pub fn best_offer_price(&self) -> Option<&Money> {
        self.best_offer_price.as_ref()
    }
    pub fn best_offer_quantity(&self) -> Option<Count> {
        self.best_offer_quantity
    }
    pub fn open(&self) -> Option<&Money> {
        self.open.as_ref()
    }
    pub fn high(&self) -> Option<&Money> {
        self.high.as_ref()
    }
    pub fn low(&self) -> Option<&Money> {
        self.low.as_ref()
    }
    pub fn previous_close(&self) -> Option<&Money> {
        self.previous_close.as_ref()
    }
    pub fn open_interest(&self) -> Option<&Money> {
        self.open_interest.as_ref()
    }
    pub fn change_in_open_interest(&self) -> Option<&Money> {
        self.change_in_open_interest.as_ref()
    }
    pub fn extra(&self) -> &BTreeMap<String, Value> {
        &self.extra
    }
}

#[derive(Clone, Debug)]
pub struct QuoteRequest {
    instrument: Instrument,
}
impl QuoteRequest {
    pub fn new(instrument: Instrument) -> Self {
        Self { instrument }
    }
}
impl EndpointRequest for QuoteRequest {
    type Response = Vec<Quote>;
    fn operation(&self) -> &'static str {
        "quotes"
    }
    fn method(&self) -> Method {
        Method::GET
    }
    fn path(&self) -> &'static str {
        "/quotes"
    }
    fn body(&self) -> Result<Vec<u8>, Error> {
        #[derive(Serialize)]
        struct Body<'a> {
            stock_code: &'a str,
            exchange_code: &'a str,
            product_type: &'a str,
        }
        compact_json(&Body {
            stock_code: self.instrument.stock_code().as_str(),
            exchange_code: self.instrument.exchange_wire(),
            product_type: self.instrument.product().wire(),
        })
    }
}

#[derive(Clone, Debug)]
pub struct OptionChainRequest {
    exchange: DerivativeExchange,
    stock_code: StockCode,
    expiry: Option<NaiveDate>,
    right: Option<OptionRight>,
    strike: Option<Money>,
}

#[derive(Clone, Debug)]
pub struct OptionChainBuilder(OptionChainRequest);

impl OptionChainRequest {
    pub fn builder(exchange: DerivativeExchange, stock_code: StockCode) -> OptionChainBuilder {
        OptionChainBuilder(Self {
            exchange,
            stock_code,
            expiry: None,
            right: None,
            strike: None,
        })
    }

    pub fn from_instrument(instrument: Instrument) -> Self {
        let exchange = match instrument.exchange() {
            crate::domain::Exchange::Nfo => DerivativeExchange::Nfo,
            crate::domain::Exchange::Bfo => DerivativeExchange::Bfo,
            crate::domain::Exchange::Ndx => DerivativeExchange::Ndx,
            crate::domain::Exchange::Mcx => DerivativeExchange::Mcx,
            _ => DerivativeExchange::Nfo,
        };
        Self {
            exchange,
            stock_code: instrument.stock_code().clone(),
            expiry: instrument.expiry(),
            right: instrument.right(),
            strike: instrument.strike().cloned(),
        }
    }
}
impl OptionChainBuilder {
    pub fn expiry(mut self, value: NaiveDate) -> Self {
        self.0.expiry = Some(value);
        self
    }
    pub fn right(mut self, value: OptionRight) -> Self {
        self.0.right = Some(value);
        self
    }
    pub fn strike(mut self, value: Money) -> Self {
        self.0.strike = Some(value);
        self
    }
    pub fn build(self) -> Result<OptionChainRequest, ValidationError> {
        let count = self.0.expiry.is_some() as u8
            + self.0.right.is_some() as u8
            + self.0.strike.is_some() as u8;
        if count < 2 {
            Err(ValidationError::new(
                "option chain requires at least two contract filters",
            ))
        } else {
            Ok(self.0)
        }
    }
}
impl EndpointRequest for OptionChainRequest {
    type Response = Vec<Quote>;
    fn operation(&self) -> &'static str {
        "optionchain"
    }
    fn method(&self) -> Method {
        Method::GET
    }
    fn path(&self) -> &'static str {
        "/optionchain"
    }
    fn body(&self) -> Result<Vec<u8>, Error> {
        #[derive(Serialize)]
        struct Body<'a> {
            stock_code: &'a str,
            exchange_code: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            expiry_date: Option<String>,
            product_type: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            right: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            strike_price: Option<String>,
        }
        compact_json(&Body {
            stock_code: self.stock_code.as_str(),
            exchange_code: self.exchange.wire(),
            expiry_date: self.expiry.map(expiry_timestamp),
            product_type: "options",
            right: self.right.map(OptionRight::wire),
            strike_price: self.strike.as_ref().map(Money::to_wire_string),
        })
    }
}
