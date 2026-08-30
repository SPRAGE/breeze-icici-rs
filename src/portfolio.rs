use std::collections::BTreeMap;

use http::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::EndpointRequest;
use crate::domain::{DateRange, Exchange, Money, StockCode};
use crate::error::{Error, ValidationError};
use crate::request::compact_json;

impl crate::request::sealed::Sealed for HoldingsRequest {}
impl crate::request::sealed::Sealed for GetPositions {}

/// Opaque portfolio filter from the official request table. ICICI documents
/// the field but does not publish a closed vocabulary, so the SDK validates
/// only a small, non-empty, control-free representation.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PortfolioType(String);

impl PortfolioType {
    pub fn new(value: impl AsRef<str>) -> Result<Self, ValidationError> {
        let value = value.as_ref().trim();
        if value.is_empty() || value.len() > 64 || value.chars().any(char::is_control) {
            return Err(ValidationError::new("invalid portfolio type"));
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug)]
pub struct HoldingsRequest {
    exchange: Exchange,
    range: Option<DateRange>,
    stock_code: Option<StockCode>,
    portfolio_type: Option<PortfolioType>,
}
#[derive(Clone, Debug)]
pub struct HoldingsBuilder(HoldingsRequest);
impl HoldingsRequest {
    pub fn builder(exchange: Exchange) -> HoldingsBuilder {
        HoldingsBuilder(Self {
            exchange,
            range: None,
            stock_code: None,
            portfolio_type: None,
        })
    }
}
impl HoldingsBuilder {
    pub fn date_range(mut self, value: DateRange) -> Self {
        self.0.range = Some(value);
        self
    }
    pub fn stock_code(mut self, value: StockCode) -> Self {
        self.0.stock_code = Some(value);
        self
    }
    pub fn portfolio_type(mut self, value: PortfolioType) -> Self {
        self.0.portfolio_type = Some(value);
        self
    }
    pub fn build(self) -> Result<HoldingsRequest, ValidationError> {
        Ok(self.0)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Holding {
    stock_code: StockCode,
    exchange_code: String,
    #[serde(default)]
    quantity: Option<String>,
    #[serde(default)]
    average_price: Option<Money>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}
impl Holding {
    pub fn stock_code(&self) -> &StockCode {
        &self.stock_code
    }
    pub fn exchange_code(&self) -> &str {
        &self.exchange_code
    }
    pub fn quantity_raw(&self) -> Option<&str> {
        self.quantity.as_deref()
    }
    pub fn average_price(&self) -> Option<&Money> {
        self.average_price.as_ref()
    }
    pub fn extra(&self) -> &BTreeMap<String, Value> {
        &self.extra
    }
}

impl EndpointRequest for HoldingsRequest {
    type Response = Vec<Holding>;
    fn operation(&self) -> &'static str {
        "portfolioholdings"
    }
    fn method(&self) -> Method {
        Method::GET
    }
    fn path(&self) -> &'static str {
        "/portfolioholdings"
    }
    fn body(&self) -> Result<Vec<u8>, Error> {
        #[derive(Serialize)]
        struct Body<'a> {
            exchange_code: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            from_date: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            to_date: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            stock_code: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            portfolio_type: Option<&'a str>,
        }
        compact_json(&Body {
            exchange_code: self.exchange.wire(),
            from_date: self.range.as_ref().map(DateRange::start_wire),
            to_date: self.range.as_ref().map(DateRange::end_wire),
            stock_code: self.stock_code.as_ref().map(StockCode::as_str),
            portfolio_type: self.portfolio_type.as_ref().map(PortfolioType::as_str),
        })
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct GetPositions;

#[derive(Clone, Debug, Deserialize)]
pub struct Position {
    stock_code: StockCode,
    exchange_code: String,
    #[serde(default)]
    quantity: Option<String>,
    #[serde(default)]
    ltp: Option<Money>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}
impl Position {
    pub fn stock_code(&self) -> &StockCode {
        &self.stock_code
    }
    pub fn exchange_code(&self) -> &str {
        &self.exchange_code
    }
    pub fn quantity_raw(&self) -> Option<&str> {
        self.quantity.as_deref()
    }
    pub fn last_price(&self) -> Option<&Money> {
        self.ltp.as_ref()
    }
    pub fn extra(&self) -> &BTreeMap<String, Value> {
        &self.extra
    }
}

impl EndpointRequest for GetPositions {
    type Response = Vec<Position>;
    fn operation(&self) -> &'static str {
        "portfoliopositions"
    }
    fn method(&self) -> Method {
        Method::GET
    }
    fn path(&self) -> &'static str {
        "/portfoliopositions"
    }
    fn body(&self) -> Result<Vec<u8>, Error> {
        Ok(b"{}".to_vec())
    }
}
