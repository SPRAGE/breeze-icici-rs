use std::collections::BTreeMap;

use http::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::EndpointRequest;
use crate::domain::{DateRange, Exchange, Product, StockCode};
use crate::error::{Error, ValidationError};
use crate::orders::{Action, OrderId};
use crate::request::compact_json;

impl crate::request::sealed::Sealed for TradeListRequest {}
impl crate::request::sealed::Sealed for TradeDetailRequest {}

#[derive(Clone, Debug)]
pub struct TradeListRequest {
    exchange: Exchange,
    range: DateRange,
    product: Option<Product>,
    action: Option<Action>,
    stock_code: Option<StockCode>,
}
#[derive(Clone, Debug)]
pub struct TradeListBuilder(TradeListRequest);
impl TradeListRequest {
    pub fn builder(exchange: Exchange, range: DateRange) -> TradeListBuilder {
        TradeListBuilder(Self {
            exchange,
            range,
            product: None,
            action: None,
            stock_code: None,
        })
    }
}
impl TradeListBuilder {
    pub fn product(mut self, value: Product) -> Self {
        self.0.product = Some(value);
        self
    }
    pub fn action(mut self, value: Action) -> Self {
        self.0.action = Some(value);
        self
    }
    pub fn stock_code(mut self, value: StockCode) -> Self {
        self.0.stock_code = Some(value);
        self
    }
    pub fn build(self) -> Result<TradeListRequest, ValidationError> {
        Ok(self.0)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Trade {
    stock_code: StockCode,
    exchange_code: String,
    order_id: OrderId,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}
impl Trade {
    pub fn stock_code(&self) -> &StockCode {
        &self.stock_code
    }
    pub fn order_id(&self) -> &OrderId {
        &self.order_id
    }
    pub fn exchange_code(&self) -> &str {
        &self.exchange_code
    }
    pub fn extra(&self) -> &BTreeMap<String, Value> {
        &self.extra
    }
}

impl EndpointRequest for TradeListRequest {
    type Response = Vec<Trade>;
    fn operation(&self) -> &'static str {
        "trades"
    }
    fn method(&self) -> Method {
        Method::GET
    }
    fn path(&self) -> &'static str {
        "/trades"
    }
    fn body(&self) -> Result<Vec<u8>, Error> {
        #[derive(Serialize)]
        struct Body<'a> {
            from_date: String,
            to_date: String,
            exchange_code: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            product_type: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            action: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            stock_code: Option<&'a str>,
        }
        compact_json(&Body {
            from_date: self.range.start_wire(),
            to_date: self.range.end_wire(),
            exchange_code: self.exchange.wire(),
            product_type: self.product.map(Product::wire),
            action: self.action.map(Action::wire),
            stock_code: self.stock_code.as_ref().map(StockCode::as_str),
        })
    }
}

#[derive(Clone, Debug)]
pub struct TradeDetailRequest {
    exchange: Exchange,
    order_id: OrderId,
}
impl TradeDetailRequest {
    pub fn new(exchange: Exchange, order_id: OrderId) -> Self {
        Self { exchange, order_id }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct TradeExecution {
    trade_id: String,
    stock_code: StockCode,
    exchange_code: String,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}
impl TradeExecution {
    pub fn trade_id(&self) -> &str {
        &self.trade_id
    }
    pub fn stock_code(&self) -> &StockCode {
        &self.stock_code
    }
    pub fn exchange_code(&self) -> &str {
        &self.exchange_code
    }
    pub fn extra(&self) -> &BTreeMap<String, Value> {
        &self.extra
    }
}

impl EndpointRequest for TradeDetailRequest {
    type Response = Vec<TradeExecution>;
    fn operation(&self) -> &'static str {
        "trades"
    }
    fn method(&self) -> Method {
        Method::GET
    }
    fn path(&self) -> &'static str {
        "/trades"
    }
    fn body(&self) -> Result<Vec<u8>, Error> {
        #[derive(Serialize)]
        struct Body<'a> {
            exchange_code: &'a str,
            order_id: &'a str,
        }
        compact_json(&Body {
            exchange_code: self.exchange.wire(),
            order_id: self.order_id.as_str(),
        })
    }
}
