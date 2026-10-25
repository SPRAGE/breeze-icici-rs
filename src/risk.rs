use http::Method;
use serde::{Deserialize, Serialize};

use crate::EndpointRequest;
use crate::domain::{Count, Instrument, Money, Product, Quantity, expiry_timestamp};
use crate::error::{Error, ValidationError};
use crate::orders::{Action, OrderId, OrderType};
use crate::request::compact_json;

impl crate::request::sealed::Sealed for MarginCalculationRequest {}
impl crate::request::sealed::Sealed for LimitPriceRequest {}

#[derive(Clone, Debug)]
pub struct MarginPosition {
    instrument: Instrument,
    action: Action,
    quantity: Quantity,
    price: Money,
    cover_order_flow: Option<Action>,
    fresh_order_type: Option<OrderType>,
    cover_limit_rate: Option<Money>,
    cover_sltp_price: Option<Money>,
    fresh_limit_rate: Option<Money>,
    open_quantity: Count,
    include_cover_fields: bool,
}
impl MarginPosition {
    pub fn new(instrument: Instrument, action: Action, quantity: Quantity, price: Money) -> Self {
        Self {
            instrument,
            action,
            quantity,
            price,
            cover_order_flow: None,
            fresh_order_type: None,
            cover_limit_rate: None,
            cover_sltp_price: None,
            fresh_limit_rate: None,
            open_quantity: Count::new(0),
            include_cover_fields: false,
        }
    }

    /// Includes all six cover fields, using N/zero for fields not explicitly set.
    pub fn include_cover_fields(mut self) -> Self {
        self.include_cover_fields = true;
        self
    }

    pub fn cover_order_flow(mut self, value: Action) -> Self {
        self.include_cover_fields = true;
        self.cover_order_flow = Some(value);
        self
    }
    pub fn fresh_order_type(mut self, value: OrderType) -> Self {
        self.include_cover_fields = true;
        self.fresh_order_type = Some(value);
        self
    }
    pub fn cover_limit_rate(mut self, value: Money) -> Self {
        self.include_cover_fields = true;
        self.cover_limit_rate = Some(value);
        self
    }
    pub fn cover_stop_loss_trigger(mut self, value: Money) -> Self {
        self.include_cover_fields = true;
        self.cover_sltp_price = Some(value);
        self
    }
    pub fn fresh_limit_rate(mut self, value: Money) -> Self {
        self.include_cover_fields = true;
        self.fresh_limit_rate = Some(value);
        self
    }
    pub fn open_quantity(mut self, value: impl Into<Count>) -> Self {
        self.include_cover_fields = true;
        self.open_quantity = value.into();
        self
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
            #[serde(skip_serializing_if = "Option::is_none")]
            cover_order_flow: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            fresh_order_type: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            cover_limit_rate: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            cover_sltp_price: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            fresh_limit_rate: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            open_quantity: Option<String>,
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
                cover_order_flow: position
                    .include_cover_fields
                    .then(|| position.cover_order_flow.map_or("N", Action::wire)),
                fresh_order_type: position
                    .include_cover_fields
                    .then(|| position.fresh_order_type.map_or("N", OrderType::wire)),
                cover_limit_rate: position.include_cover_fields.then(|| {
                    position
                        .cover_limit_rate
                        .as_ref()
                        .map_or_else(|| "0".into(), Money::to_wire_string)
                }),
                cover_sltp_price: position.include_cover_fields.then(|| {
                    position
                        .cover_sltp_price
                        .as_ref()
                        .map_or_else(|| "0".into(), Money::to_wire_string)
                }),
                fresh_limit_rate: position.include_cover_fields.then(|| {
                    position
                        .fresh_limit_rate
                        .as_ref()
                        .map_or_else(|| "0".into(), Money::to_wire_string)
                }),
                open_quantity: position
                    .include_cover_fields
                    .then(|| position.open_quantity.get().to_string()),
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
    /// The SDK's M selector; its upstream business meaning is unspecified.
    M,
}
impl SourceFlag {
    fn wire(self) -> &'static str {
        match self {
            Self::Portfolio => "P",
            Self::Other => "O",
            Self::M => "M",
        }
    }
}

#[derive(Clone, Debug)]
pub struct LimitPriceRequest {
    instrument: Instrument,
    product_type: Product,
    action: Action,
    stop_loss_trigger: Money,
    source_flag: SourceFlag,
    limit_rate: Option<Money>,
    order_reference: Option<OrderId>,
    available_quantity: Option<Count>,
    fresh_order_limit: Money,
}
#[derive(Clone, Debug)]
pub struct LimitPriceBuilder {
    instrument: Instrument,
    product_type: Product,
    action: Action,
    stop_loss_trigger: Option<Money>,
    source_flag: Option<SourceFlag>,
    limit_rate: Option<Money>,
    order_reference: Option<OrderId>,
    available_quantity: Option<Count>,
    fresh_order_limit: Option<Money>,
}
impl LimitPriceRequest {
    pub fn builder(instrument: Instrument, action: Action) -> LimitPriceBuilder {
        LimitPriceBuilder {
            product_type: instrument.product(),
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
    /// Overrides the inferred instrument product.
    pub fn product_type(mut self, value: Product) -> Self {
        self.product_type = value;
        self
    }
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
    /// Accepts zero for calculator inputs; order quantities remain positive.
    pub fn available_quantity(mut self, value: impl Into<Count>) -> Self {
        self.available_quantity = Some(value.into());
        self
    }
    pub fn fresh_order_limit(mut self, value: Money) -> Self {
        self.fresh_order_limit = Some(value);
        self
    }
    pub fn build(self) -> Result<LimitPriceRequest, ValidationError> {
        if self.product_type != self.instrument.product() {
            return Err(ValidationError::new(
                "calculator product must match the instrument",
            ));
        }
        Ok(LimitPriceRequest {
            instrument: self.instrument,
            product_type: self.product_type,
            action: self.action,
            stop_loss_trigger: self
                .stop_loss_trigger
                .ok_or_else(|| ValidationError::new("stop-loss trigger is required"))?,
            source_flag: self
                .source_flag
                .ok_or_else(|| ValidationError::new("source flag is required"))?,
            limit_rate: self.limit_rate,
            order_reference: self.order_reference,
            available_quantity: self.available_quantity,
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
            product_type: self.product_type.wire(),
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
            limit_rate: self
                .limit_rate
                .as_ref()
                .map_or_else(String::new, Money::to_wire_string),
            order_reference: self.order_reference.as_ref().map_or("", OrderId::as_str),
            available_quantity: self
                .available_quantity
                .map_or_else(String::new, |value| value.get().to_string()),
            market_type: "limit",
            fresh_order_limit: self.fresh_order_limit.to_wire_string(),
        })
    }
}
