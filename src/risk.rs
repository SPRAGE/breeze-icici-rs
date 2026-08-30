use http::Method;
use serde::{Deserialize, Serialize};

use crate::EndpointRequest;
use crate::domain::{Instrument, Money, Quantity, expiry_timestamp};
use crate::error::{Error, ValidationError};
use crate::orders::{Action, OrderId};
use crate::request::compact_json;

impl crate::request::sealed::Sealed for MarginCalculationRequest {}
impl crate::request::sealed::Sealed for LimitPriceRequest {}

#[derive(Clone, Debug)]
pub struct MarginPosition {
    instrument: Instrument,
    action: Action,
    quantity: Quantity,
    price: Money,
}
impl MarginPosition {
    pub fn new(instrument: Instrument, action: Action, quantity: Quantity, price: Money) -> Self {
        Self {
            instrument,
            action,
            quantity,
            price,
        }
    }
}

#[derive(Clone, Debug)]
pub struct MarginCalculationRequest {
    positions: Vec<MarginPosition>,
}
impl MarginCalculationRequest {
    pub fn new(positions: Vec<MarginPosition>) -> Result<Self, ValidationError> {
        if positions.is_empty() {
            Err(ValidationError::new("at least one position is required"))
        } else {
            Ok(Self { positions })
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct MarginCalculation {
    #[serde(default)]
    pub margin_calulation: Vec<serde_json::Value>,
    #[serde(default)]
    pub non_span_margin_required: Option<Money>,
    #[serde(default)]
    pub order_value: Option<Money>,
    #[serde(default)]
    pub order_margin: Option<Money>,
    #[serde(default)]
    pub trade_margin: Option<Money>,
    #[serde(default)]
    pub block_trade_margin: Option<Money>,
    #[serde(default)]
    pub span_margin_required: Option<Money>,
}

impl EndpointRequest for MarginCalculationRequest {
    type Response = MarginCalculation;
    fn operation(&self) -> &'static str {
        "margincalculator"
    }
    fn method(&self) -> Method {
        Method::POST
    }
    fn path(&self) -> &'static str {
        "/margincalculator"
    }
    fn body(&self) -> Result<Vec<u8>, Error> {
        #[derive(Serialize)]
        struct Position<'a> {
            strike_price: String,
            quantity: String,
            right: &'a str,
            product: &'a str,
            action: &'a str,
            price: String,
            expiry_date: String,
            stock_code: &'a str,
        }
        #[derive(Serialize)]
        struct Body<'a> {
            list_of_positions: Vec<Position<'a>>,
            exchange_code: &'a str,
        }
        let exchange = self.positions[0].instrument.exchange_wire();
        if self
            .positions
            .iter()
            .any(|position| position.instrument.exchange_wire() != exchange)
        {
            return Err(
                ValidationError::new("all margin positions must use the same exchange").into(),
            );
        }
        let positions = self
            .positions
            .iter()
            .map(|position| Position {
                strike_price: position
                    .instrument
                    .strike()
                    .map_or_else(|| "0".into(), Money::to_wire_string),
                quantity: position.quantity.get().to_string(),
                right: position
                    .instrument
                    .right()
                    .map_or("others", |value| value.wire()),
                product: position.instrument.product().wire(),
                action: position.action.wire(),
                price: position.price.to_wire_string(),
                expiry_date: position
                    .instrument
                    .expiry()
                    .map(expiry_timestamp)
                    .unwrap_or_default(),
                stock_code: position.instrument.stock_code().as_str(),
            })
            .collect();
        compact_json(&Body {
            list_of_positions: positions,
            exchange_code: exchange,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceFlag {
    Portfolio,
    Other,
}
impl SourceFlag {
    fn wire(self) -> &'static str {
        match self {
            Self::Portfolio => "P",
            Self::Other => "O",
        }
    }
}

#[derive(Clone, Debug)]
pub struct LimitPriceRequest {
    instrument: Instrument,
    action: Action,
    stop_loss_trigger: Money,
    source_flag: SourceFlag,
    limit_rate: Money,
    order_reference: OrderId,
    available_quantity: Quantity,
    fresh_order_limit: Money,
}
#[derive(Clone, Debug)]
pub struct LimitPriceBuilder {
    instrument: Instrument,
    action: Action,
    stop_loss_trigger: Option<Money>,
    source_flag: Option<SourceFlag>,
    limit_rate: Option<Money>,
    order_reference: Option<OrderId>,
    available_quantity: Option<Quantity>,
    fresh_order_limit: Option<Money>,
}
impl LimitPriceRequest {
    pub fn builder(instrument: Instrument, action: Action) -> LimitPriceBuilder {
        LimitPriceBuilder {
            instrument,
            action,
            stop_loss_trigger: None,
            source_flag: None,
            limit_rate: None,
            order_reference: None,
            available_quantity: None,
            fresh_order_limit: None,
        }
    }
}
impl LimitPriceBuilder {
    pub fn stop_loss_trigger(mut self, value: Money) -> Self {
        self.stop_loss_trigger = Some(value);
        self
    }
    pub fn source_flag(mut self, value: SourceFlag) -> Self {
        self.source_flag = Some(value);
        self
    }
    pub fn limit_rate(mut self, value: Money) -> Self {
        self.limit_rate = Some(value);
        self
    }
    pub fn order_reference(mut self, value: OrderId) -> Self {
        self.order_reference = Some(value);
        self
    }
    pub fn available_quantity(mut self, value: Quantity) -> Self {
        self.available_quantity = Some(value);
        self
    }
    pub fn fresh_order_limit(mut self, value: Money) -> Self {
        self.fresh_order_limit = Some(value);
        self
    }
    pub fn build(self) -> Result<LimitPriceRequest, ValidationError> {
        Ok(LimitPriceRequest {
            instrument: self.instrument,
            action: self.action,
            stop_loss_trigger: self
                .stop_loss_trigger
                .ok_or_else(|| ValidationError::new("stop-loss trigger is required"))?,
            source_flag: self
                .source_flag
                .ok_or_else(|| ValidationError::new("source flag is required"))?,
            limit_rate: self
                .limit_rate
                .ok_or_else(|| ValidationError::new("limit rate is required"))?,
            order_reference: self
                .order_reference
                .ok_or_else(|| ValidationError::new("order reference is required"))?,
            available_quantity: self
                .available_quantity
                .ok_or_else(|| ValidationError::new("available quantity is required"))?,
            fresh_order_limit: self
                .fresh_order_limit
                .ok_or_else(|| ValidationError::new("fresh-order limit is required"))?,
        })
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct LimitPriceResult {
    pub available_quantity: String,
    pub action_id: String,
    pub order_margin: Money,
    pub limit_rate: Money,
}
impl EndpointRequest for LimitPriceRequest {
    type Response = LimitPriceResult;
    fn operation(&self) -> &'static str {
        "fnolmtpriceandqtycal"
    }
    fn method(&self) -> Method {
        Method::POST
    }
    fn path(&self) -> &'static str {
        "/fnolmtpriceandqtycal"
    }
    fn body(&self) -> Result<Vec<u8>, Error> {
        #[derive(Serialize)]
        struct Body<'a> {
            strike_price: String,
            product_type: &'a str,
            expiry_date: String,
            underlying: &'a str,
            exchange_code: &'a str,
            order_flow: &'a str,
            stop_loss_trigger: String,
            option_type: &'a str,
            source_flag: &'a str,
            limit_rate: String,
            order_reference: &'a str,
            available_quantity: String,
            market_type: &'a str,
            fresh_order_limit: String,
        }
        compact_json(&Body {
            strike_price: self
                .instrument
                .strike()
                .map_or_else(|| "0".into(), Money::to_wire_string),
            product_type: self.instrument.product().wire(),
            expiry_date: self
                .instrument
                .expiry()
                .map(expiry_timestamp)
                .unwrap_or_default(),
            underlying: self.instrument.stock_code().as_str(),
            exchange_code: self.instrument.exchange_wire(),
            order_flow: self.action.wire(),
            stop_loss_trigger: self.stop_loss_trigger.to_wire_string(),
            option_type: self.instrument.right().map_or("others", |v| v.wire()),
            source_flag: self.source_flag.wire(),
            limit_rate: self.limit_rate.to_wire_string(),
            order_reference: self.order_reference.as_str(),
            available_quantity: self.available_quantity.get().to_string(),
            market_type: "limit",
            fresh_order_limit: self.fresh_order_limit.to_wire_string(),
        })
    }
}
