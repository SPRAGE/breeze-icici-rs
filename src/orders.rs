use std::collections::BTreeMap;
use std::str::FromStr;

use http::Method;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::EndpointRequest;
use crate::domain::{
    DateRange, Exchange, Instrument, Money, Quantity, UnknownValue, UserRemark, expiry_timestamp,
};
use crate::error::{Error, ValidationError};
use crate::rate_limit::RequestClass;
use crate::request::compact_json;

impl crate::request::sealed::Sealed for PlaceOrderRequest {}
impl crate::request::sealed::Sealed for OrderDetailRequest {}
impl crate::request::sealed::Sealed for OrderListRequest {}
impl crate::request::sealed::Sealed for CancelOrderRequest {}
impl crate::request::sealed::Sealed for ModifyOrderRequest {}
impl crate::request::sealed::Sealed for SquareOffRequest {}
impl crate::request::sealed::Sealed for PreviewOrderRequest {}

macro_rules! id_type {
    ($name:ident, $label:literal) => {
        #[derive(Clone, Debug, Eq, Hash, PartialEq)]
        pub struct $name(String);
        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, ValidationError> {
                let value = value.into();
                if value.trim().is_empty() || value.len() > 128 {
                    Err(ValidationError::new(concat!($label, " is invalid")))
                } else {
                    Ok(Self(value))
                }
            }
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
            }
        }
    };
}

id_type!(OrderId, "order ID");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Buy,
    Sell,
}
impl Action {
    pub(crate) fn wire(self) -> &'static str {
        match self {
            Self::Buy => "buy",
            Self::Sell => "sell",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrderType {
    Limit,
    StopLoss,
}
impl OrderType {
    fn wire(self) -> &'static str {
        match self {
            Self::Limit => "limit",
            Self::StopLoss => "stoploss",
        }
    }
}
impl FromStr for OrderType {
    type Err = ValidationError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "limit" => Ok(Self::Limit),
            "stoploss" | "stop_loss" => Ok(Self::StopLoss),
            _ => Err(ValidationError::new("unsupported order type")),
        }
    }
}

fn validate_stop_loss_relation(
    action: Action,
    limit: &Money,
    trigger: &Money,
) -> Result<(), ValidationError> {
    let valid = match action {
        Action::Buy => limit.decimal() >= trigger.decimal(),
        Action::Sell => limit.decimal() <= trigger.decimal(),
    };
    if valid {
        Ok(())
    } else {
        Err(ValidationError::new(
            "buy stop-loss limit must be at or above its trigger and sell limit at or below it",
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Validity {
    Day,
    ImmediateOrCancel,
}
impl Validity {
    fn wire(self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::ImmediateOrCancel => "ioc",
        }
    }
}

#[derive(Clone, Debug)]
pub struct PlaceOrderRequest {
    instrument: Instrument,
    action: Action,
    order_type: OrderType,
    quantity: Quantity,
    price: Money,
    stop_loss: Option<Money>,
    validity: Validity,
    disclosed_quantity: Option<Quantity>,
    user_remark: Option<UserRemark>,
}

#[derive(Clone, Debug)]
pub struct PlaceOrderBuilder {
    instrument: Instrument,
    action: Action,
    quantity: Quantity,
    price: Money,
    order_type: OrderType,
    stop_loss: Option<Money>,
    validity: Option<Validity>,
    disclosed_quantity: Option<Quantity>,
    user_remark: Option<UserRemark>,
}

impl PlaceOrderRequest {
    pub fn limit(
        instrument: Instrument,
        action: Action,
        quantity: Quantity,
        price: Money,
    ) -> PlaceOrderBuilder {
        PlaceOrderBuilder {
            instrument,
            action,
            quantity,
            price,
            order_type: OrderType::Limit,
            stop_loss: None,
            validity: None,
            disclosed_quantity: None,
            user_remark: None,
        }
    }

    /// Creates a stop-loss-limit order. For buys, `price` must be at least the
    /// trigger; for sells, it must be no greater than the trigger.
    pub fn stop_loss(
        instrument: Instrument,
        action: Action,
        quantity: Quantity,
        price: Money,
        trigger: Money,
    ) -> PlaceOrderBuilder {
        PlaceOrderBuilder {
            instrument,
            action,
            quantity,
            price,
            order_type: OrderType::StopLoss,
            stop_loss: Some(trigger),
            validity: None,
            disclosed_quantity: None,
            user_remark: None,
        }
    }
    pub fn instrument(&self) -> &Instrument {
        &self.instrument
    }
    pub fn action(&self) -> Action {
        self.action
    }
    pub fn order_type(&self) -> OrderType {
        self.order_type
    }
    pub fn stop_loss_trigger(&self) -> Option<&Money> {
        self.stop_loss.as_ref()
    }
}
impl PlaceOrderBuilder {
    pub fn validity(mut self, value: Validity) -> Self {
        self.validity = Some(value);
        self
    }
    pub fn user_remark(mut self, value: UserRemark) -> Self {
        self.user_remark = Some(value);
        self
    }
    pub fn disclosed_quantity(mut self, value: Quantity) -> Self {
        self.disclosed_quantity = Some(value);
        self
    }
    pub fn build(self) -> Result<PlaceOrderRequest, ValidationError> {
        if let Some(trigger) = &self.stop_loss {
            validate_stop_loss_relation(self.action, &self.price, trigger)?;
        }
        if self
            .disclosed_quantity
            .is_some_and(|value| value.get() > self.quantity.get())
        {
            return Err(ValidationError::new(
                "disclosed quantity must not exceed order quantity",
            ));
        }
        Ok(PlaceOrderRequest {
            instrument: self.instrument,
            action: self.action,
            order_type: self.order_type,
            quantity: self.quantity,
            price: self.price,
            stop_loss: self.stop_loss,
            validity: self
                .validity
                .ok_or_else(|| ValidationError::new("validity is required"))?,
            disclosed_quantity: self.disclosed_quantity,
            user_remark: self.user_remark,
        })
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct OrderReceipt {
    #[serde(alias = "gtt_order_id")]
    order_id: OrderId,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    user_remark: Option<String>,
    #[serde(default)]
    indicator: Option<String>,
}
impl OrderReceipt {
    pub fn order_id(&self) -> &OrderId {
        &self.order_id
    }
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }
    pub fn user_remark(&self) -> Option<&str> {
        self.user_remark.as_deref()
    }
    pub fn indicator(&self) -> Option<&str> {
        self.indicator.as_deref()
    }
}

impl EndpointRequest for PlaceOrderRequest {
    type Response = OrderReceipt;
    fn operation(&self) -> &'static str {
        "order"
    }
    fn method(&self) -> Method {
        Method::POST
    }
    fn path(&self) -> &'static str {
        "/order"
    }
    fn request_class(&self) -> RequestClass {
        RequestClass::PlaceOrder
    }
    fn body(&self) -> Result<Vec<u8>, Error> {
        #[derive(Serialize)]
        struct EquityBody<'a> {
            stock_code: &'a str,
            exchange_code: &'a str,
            product: &'a str,
            action: &'a str,
            order_type: &'a str,
            quantity: String,
            price: String,
            validity: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            stoploss: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            disclosed_quantity: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            user_remark: Option<&'a str>,
        }
        #[derive(Serialize)]
        struct DerivativeBody<'a> {
            stock_code: &'a str,
            exchange_code: &'a str,
            product: &'a str,
            action: &'a str,
            order_type: &'a str,
            quantity: String,
            price: String,
            validity: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            stoploss: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            disclosed_quantity: Option<String>,
            expiry_date: String,
            right: &'a str,
            strike_price: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            user_remark: Option<&'a str>,
        }
        let common_remark = self.user_remark.as_ref().map(UserRemark::as_str);
        if let Some(expiry) = self.instrument.expiry() {
            compact_json(&DerivativeBody {
                stock_code: self.instrument.stock_code().as_str(),
                exchange_code: self.instrument.exchange_wire(),
                product: self.instrument.product().wire(),
                action: self.action.wire(),
                order_type: self.order_type.wire(),
                quantity: self.quantity.get().to_string(),
                price: self.price.to_wire_string(),
                validity: self.validity.wire(),
                stoploss: self.stop_loss.as_ref().map(Money::to_wire_string),
                disclosed_quantity: self.disclosed_quantity.map(|value| value.get().to_string()),
                expiry_date: expiry_timestamp(expiry),
                right: self
                    .instrument
                    .right()
                    .map_or("others", |right| right.wire()),
                strike_price: self
                    .instrument
                    .strike()
                    .map_or_else(|| "0".into(), Money::to_wire_string),
                user_remark: common_remark,
            })
        } else {
            compact_json(&EquityBody {
                stock_code: self.instrument.stock_code().as_str(),
                exchange_code: self.instrument.exchange_wire(),
                product: self.instrument.product().wire(),
                action: self.action.wire(),
                order_type: self.order_type.wire(),
                quantity: self.quantity.get().to_string(),
                price: self.price.to_wire_string(),
                validity: self.validity.wire(),
                stoploss: self.stop_loss.as_ref().map(Money::to_wire_string),
                disclosed_quantity: self.disclosed_quantity.map(|value| value.get().to_string()),
                user_remark: common_remark,
            })
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OrderStatus {
    Ordered,
    Requested,
    Executed,
    Cancelled,
    Rejected,
    Other(UnknownValue),
}

impl OrderStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Executed | Self::Cancelled | Self::Rejected)
    }
}
impl<'de> Deserialize<'de> for OrderStatus {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Ok(match value.to_ascii_lowercase().as_str() {
            "ordered" => Self::Ordered,
            "requested" => Self::Requested,
            "executed" | "complete" | "completed" => Self::Executed,
            "cancelled" | "canceled" => Self::Cancelled,
            "rejected" => Self::Rejected,
            _ => Self::Other(UnknownValue::new(value)),
        })
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Order {
    order_id: OrderId,
    #[serde(default)]
    exchange_order_id: Option<String>,
    exchange_code: String,
    stock_code: String,
    #[serde(default)]
    expiry_date: Option<String>,
    #[serde(default)]
    parent_order_id: Option<String>,
    #[serde(default)]
    validity_date: Option<String>,
    status: OrderStatus,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}
impl Order {
    pub fn order_id(&self) -> &OrderId {
        &self.order_id
    }
    pub fn expiry(&self) -> Option<&str> {
        self.expiry_date.as_deref()
    }
    pub fn exchange_order_id(&self) -> Option<&str> {
        self.exchange_order_id.as_deref()
    }
    pub fn parent_order_id(&self) -> Option<&str> {
        self.parent_order_id.as_deref()
    }
    pub fn validity_date_raw(&self) -> Option<&str> {
        self.validity_date.as_deref()
    }
    pub fn status(&self) -> &OrderStatus {
        &self.status
    }
    pub fn exchange_code(&self) -> &str {
        &self.exchange_code
    }
    pub fn stock_code(&self) -> &str {
        &self.stock_code
    }
    pub fn extra(&self) -> &BTreeMap<String, Value> {
        &self.extra
    }
}

#[derive(Clone, Debug)]
pub struct OrderDetailRequest {
    exchange: Exchange,
    order_id: OrderId,
}
impl OrderDetailRequest {
    pub fn new(exchange: Exchange, order_id: OrderId) -> Self {
        Self { exchange, order_id }
    }
}
impl EndpointRequest for OrderDetailRequest {
    type Response = Vec<Order>;
    fn operation(&self) -> &'static str {
        "order"
    }
    fn method(&self) -> Method {
        Method::GET
    }
    fn path(&self) -> &'static str {
        "/order"
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

#[derive(Clone, Debug)]
pub struct OrderListRequest {
    exchange: Exchange,
    range: DateRange,
}
impl OrderListRequest {
    pub fn new(exchange: Exchange, range: DateRange) -> Result<Self, ValidationError> {
        if range.to() - range.from() > chrono::Duration::days(10) {
            return Err(ValidationError::new(
                "order list range may not exceed ten days",
            ));
        }
        Ok(Self { exchange, range })
    }
}
impl EndpointRequest for OrderListRequest {
    type Response = Vec<Order>;
    fn operation(&self) -> &'static str {
        "order"
    }
    fn method(&self) -> Method {
        Method::GET
    }
    fn path(&self) -> &'static str {
        "/order"
    }
    fn body(&self) -> Result<Vec<u8>, Error> {
        #[derive(Serialize)]
        struct Body<'a> {
            exchange_code: &'a str,
            from_date: String,
            to_date: String,
        }
        compact_json(&Body {
            exchange_code: self.exchange.wire(),
            from_date: self.range.start_wire(),
            to_date: self.range.end_wire(),
        })
    }
}

#[derive(Clone, Debug)]
pub struct CancelOrderRequest {
    order_id: OrderId,
    exchange: Exchange,
}
impl CancelOrderRequest {
    pub fn new(order_id: OrderId, exchange: Exchange) -> Self {
        Self { order_id, exchange }
    }
}
impl EndpointRequest for CancelOrderRequest {
    type Response = OrderReceipt;
    fn operation(&self) -> &'static str {
        "order"
    }
    fn method(&self) -> Method {
        Method::DELETE
    }
    fn path(&self) -> &'static str {
        "/order"
    }
    fn request_class(&self) -> RequestClass {
        RequestClass::CancelOrder
    }
    fn body(&self) -> Result<Vec<u8>, Error> {
        #[derive(Serialize)]
        struct Body<'a> {
            order_id: &'a str,
            exchange_code: &'a str,
        }
        compact_json(&Body {
            order_id: self.order_id.as_str(),
            exchange_code: self.exchange.wire(),
        })
    }
}

#[derive(Clone, Debug)]
pub struct ModifyOrderRequest {
    exchange: Exchange,
    order_id: OrderId,
    quantity: Option<Quantity>,
    price: Option<Money>,
    order_type: Option<OrderType>,
    stop_loss: Option<Money>,
    validity: Option<Validity>,
    disclosed_quantity: Option<Quantity>,
}
#[derive(Clone, Debug)]
pub struct ModifyOrderBuilder {
    exchange: Exchange,
    order_id: OrderId,
    quantity: Option<Quantity>,
    price: Option<Money>,
    order_type: Option<OrderType>,
    stop_loss: Option<Money>,
    validity: Option<Validity>,
    disclosed_quantity: Option<Quantity>,
}
impl ModifyOrderRequest {
    pub fn builder(exchange: Exchange, order_id: OrderId) -> ModifyOrderBuilder {
        ModifyOrderBuilder {
            exchange,
            order_id,
            quantity: None,
            price: None,
            order_type: None,
            stop_loss: None,
            validity: None,
            disclosed_quantity: None,
        }
    }
}
impl ModifyOrderBuilder {
    pub fn quantity(mut self, value: Quantity) -> Self {
        self.quantity = Some(value);
        self
    }
    pub fn price(mut self, value: Money) -> Self {
        self.price = Some(value);
        self
    }
    pub fn order_type(mut self, value: OrderType) -> Self {
        self.order_type = Some(value);
        self
    }
    pub fn stop_loss(mut self, value: Money) -> Self {
        self.stop_loss = Some(value);
        self
    }
    pub fn validity(mut self, value: Validity) -> Self {
        self.validity = Some(value);
        self
    }
    pub fn disclosed_quantity(mut self, value: Quantity) -> Self {
        self.disclosed_quantity = Some(value);
        self
    }
    pub fn build(self) -> Result<ModifyOrderRequest, ValidationError> {
        if self.quantity.is_none()
            && self.price.is_none()
            && self.order_type.is_none()
            && self.stop_loss.is_none()
            && self.validity.is_none()
            && self.disclosed_quantity.is_none()
        {
            return Err(ValidationError::new(
                "at least one order change is required",
            ));
        }
        if self
            .quantity
            .zip(self.disclosed_quantity)
            .is_some_and(|(quantity, disclosed)| disclosed.get() > quantity.get())
        {
            return Err(ValidationError::new(
                "disclosed quantity must not exceed order quantity",
            ));
        }
        Ok(ModifyOrderRequest {
            exchange: self.exchange,
            order_id: self.order_id,
            quantity: self.quantity,
            price: self.price,
            order_type: self.order_type,
            stop_loss: self.stop_loss,
            validity: self.validity,
            disclosed_quantity: self.disclosed_quantity,
        })
    }
}
impl EndpointRequest for ModifyOrderRequest {
    type Response = OrderReceipt;
    fn operation(&self) -> &'static str {
        "order"
    }
    fn method(&self) -> Method {
        Method::PUT
    }
    fn path(&self) -> &'static str {
        "/order"
    }
    fn request_class(&self) -> RequestClass {
        RequestClass::ModifyOrder
    }
    fn body(&self) -> Result<Vec<u8>, Error> {
        #[derive(Serialize)]
        struct Body<'a> {
            order_id: &'a str,
            exchange_code: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            quantity: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            price: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            order_type: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            stoploss: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            validity: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            disclosed_quantity: Option<String>,
        }
        compact_json(&Body {
            order_id: self.order_id.as_str(),
            exchange_code: self.exchange.wire(),
            quantity: self.quantity.map(|v| v.get().to_string()),
            price: self.price.as_ref().map(Money::to_wire_string),
            order_type: self.order_type.map(OrderType::wire),
            stoploss: self.stop_loss.as_ref().map(Money::to_wire_string),
            validity: self.validity.map(Validity::wire),
            disclosed_quantity: self.disclosed_quantity.map(|v| v.get().to_string()),
        })
    }
}

#[derive(Clone, Debug)]
pub struct SquareOffRequest {
    instrument: Instrument,
    action: Action,
    quantity: Quantity,
    price: Money,
    order_type: OrderType,
    stop_loss: Option<Money>,
    validity: Validity,
    open_quantity: Quantity,
    disclosed_quantity: Option<Quantity>,
}
#[derive(Clone, Debug)]
pub struct SquareOffBuilder {
    instrument: Instrument,
    action: Action,
    quantity: Quantity,
    price: Money,
    order_type: OrderType,
    stop_loss: Option<Money>,
    validity: Option<Validity>,
    open_quantity: Option<Quantity>,
    disclosed_quantity: Option<Quantity>,
}
impl SquareOffRequest {
    pub fn limit(
        instrument: Instrument,
        action: Action,
        quantity: Quantity,
        price: Money,
    ) -> SquareOffBuilder {
        SquareOffBuilder {
            instrument,
            action,
            quantity,
            price,
            order_type: OrderType::Limit,
            stop_loss: None,
            validity: None,
            open_quantity: None,
            disclosed_quantity: None,
        }
    }

    /// Creates a stop-loss-limit square-off request.
    pub fn stop_loss(
        instrument: Instrument,
        action: Action,
        quantity: Quantity,
        price: Money,
        trigger: Money,
    ) -> SquareOffBuilder {
        SquareOffBuilder {
            instrument,
            action,
            quantity,
            price,
            order_type: OrderType::StopLoss,
            stop_loss: Some(trigger),
            validity: None,
            open_quantity: None,
            disclosed_quantity: None,
        }
    }
}
impl SquareOffBuilder {
    pub fn validity(mut self, value: Validity) -> Self {
        self.validity = Some(value);
        self
    }
    pub fn open_quantity(mut self, value: Quantity) -> Self {
        self.open_quantity = Some(value);
        self
    }
    pub fn disclosed_quantity(mut self, value: Quantity) -> Self {
        self.disclosed_quantity = Some(value);
        self
    }
    pub fn build(self) -> Result<SquareOffRequest, ValidationError> {
        if let Some(trigger) = &self.stop_loss {
            validate_stop_loss_relation(self.action, &self.price, trigger)?;
        }
        if self
            .disclosed_quantity
            .is_some_and(|value| value.get() > self.quantity.get())
        {
            return Err(ValidationError::new(
                "disclosed quantity must not exceed order quantity",
            ));
        }
        Ok(SquareOffRequest {
            instrument: self.instrument,
            action: self.action,
            quantity: self.quantity,
            price: self.price,
            order_type: self.order_type,
            stop_loss: self.stop_loss,
            validity: self
                .validity
                .ok_or_else(|| ValidationError::new("validity is required"))?,
            open_quantity: self
                .open_quantity
                .ok_or_else(|| ValidationError::new("open quantity is required"))?,
            disclosed_quantity: self.disclosed_quantity,
        })
    }
}
impl EndpointRequest for SquareOffRequest {
    type Response = OrderReceipt;
    fn operation(&self) -> &'static str {
        "squareoff"
    }
    fn method(&self) -> Method {
        Method::POST
    }
    fn path(&self) -> &'static str {
        "/squareoff"
    }
    fn request_class(&self) -> RequestClass {
        RequestClass::SquareOff
    }
    fn body(&self) -> Result<Vec<u8>, Error> {
        #[derive(Serialize)]
        struct Body<'a> {
            source_flag: &'a str,
            stock_code: &'a str,
            exchange_code: &'a str,
            quantity: String,
            price: String,
            action: &'a str,
            order_type: &'a str,
            validity: &'a str,
            stoploss_price: String,
            disclosed_quantity: String,
            protection_percentage: &'a str,
            settlement_id: &'a str,
            margin_amount: &'a str,
            open_quantity: String,
            cover_quantity: &'a str,
            product_type: &'a str,
            expiry_date: String,
            right: &'a str,
            strike_price: String,
            alias_name: &'a str,
            trade_password: &'a str,
        }
        compact_json(&Body {
            source_flag: "",
            stock_code: self.instrument.stock_code().as_str(),
            exchange_code: self.instrument.exchange_wire(),
            quantity: self.quantity.get().to_string(),
            price: self.price.to_wire_string(),
            action: self.action.wire(),
            order_type: self.order_type.wire(),
            validity: self.validity.wire(),
            stoploss_price: self
                .stop_loss
                .as_ref()
                .map_or_else(String::new, Money::to_wire_string),
            disclosed_quantity: self
                .disclosed_quantity
                .map_or_else(|| "0".to_owned(), |value| value.get().to_string()),
            protection_percentage: "",
            settlement_id: "",
            margin_amount: "",
            open_quantity: self.open_quantity.get().to_string(),
            cover_quantity: "",
            product_type: self.instrument.product().wire(),
            expiry_date: self
                .instrument
                .expiry()
                .map(expiry_timestamp)
                .unwrap_or_default(),
            right: self.instrument.right().map_or("", |v| v.wire()),
            strike_price: self
                .instrument
                .strike()
                .map_or_else(String::new, Money::to_wire_string),
            alias_name: "",
            trade_password: "",
        })
    }
}

#[derive(Clone, Debug)]
pub struct PreviewOrderRequest {
    instrument: Instrument,
    action: Action,
    quantity: Quantity,
    price: Money,
    stop_loss: Option<Money>,
    fresh_order_rate: Option<Money>,
}
#[derive(Clone, Debug)]
pub struct PreviewOrderBuilder {
    request: PreviewOrderRequest,
}
impl PreviewOrderRequest {
    pub fn limit(
        instrument: Instrument,
        action: Action,
        quantity: Quantity,
        price: Money,
    ) -> PreviewOrderBuilder {
        PreviewOrderBuilder {
            request: PreviewOrderRequest {
                instrument,
                action,
                quantity,
                price,
                stop_loss: None,
                fresh_order_rate: None,
            },
        }
    }
}
impl PreviewOrderBuilder {
    pub fn stop_loss(mut self, value: Money) -> Self {
        self.request.stop_loss = Some(value);
        self
    }
    pub fn fresh_order_rate(mut self, value: Money) -> Self {
        self.request.fresh_order_rate = Some(value);
        self
    }
    pub fn build(self) -> Result<PreviewOrderRequest, ValidationError> {
        Ok(self.request)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct OrderPreview {
    pub brokerage: Money,
    pub exchange_turnover_charges: Money,
    pub stamp_duty: Money,
    pub stt: Money,
    pub sebi_charges: Money,
    pub gst: Money,
    pub total_turnover_and_sebi_charges: Money,
    pub total_other_charges: Money,
    pub total_brokerage: Money,
}
impl EndpointRequest for PreviewOrderRequest {
    type Response = OrderPreview;
    fn operation(&self) -> &'static str {
        "preview_order"
    }
    fn method(&self) -> Method {
        Method::GET
    }
    fn path(&self) -> &'static str {
        "/preview_order"
    }
    fn body(&self) -> Result<Vec<u8>, Error> {
        #[derive(Serialize)]
        struct Body<'a> {
            stock_code: &'a str,
            exchange_code: &'a str,
            product: &'a str,
            order_type: &'a str,
            price: String,
            action: &'a str,
            quantity: String,
            expiry_date: String,
            right: &'a str,
            strike_price: String,
            specialflag: &'a str,
            stoploss: String,
            order_rate_fresh: String,
        }
        compact_json(&Body {
            stock_code: self.instrument.stock_code().as_str(),
            exchange_code: self.instrument.exchange_wire(),
            product: self.instrument.product().wire(),
            order_type: OrderType::Limit.wire(),
            price: self.price.to_wire_string(),
            action: self.action.wire(),
            quantity: self.quantity.get().to_string(),
            expiry_date: self
                .instrument
                .expiry()
                .map(expiry_timestamp)
                .unwrap_or_default(),
            right: self.instrument.right().map_or("", |v| v.wire()),
            strike_price: self
                .instrument
                .strike()
                .map_or_else(String::new, Money::to_wire_string),
            specialflag: "",
            stoploss: self
                .stop_loss
                .as_ref()
                .map_or_else(String::new, Money::to_wire_string),
            order_rate_fresh: self
                .fresh_order_rate
                .as_ref()
                .map_or_else(String::new, Money::to_wire_string),
        })
    }
}
