use std::collections::BTreeMap;

use chrono::NaiveDate;
use http::Method;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::EndpointRequest;
use crate::domain::{DateRange, Instrument, Money, Quantity, expiry_timestamp};
use crate::error::{Error, ValidationError};
use crate::orders::Action;
use crate::rate_limit::RequestClass;
use crate::request::compact_json;

impl crate::request::sealed::Sealed for GttOrderRequest {}
impl crate::request::sealed::Sealed for GttOrderListRequest {}
impl crate::request::sealed::Sealed for CancelGttOrderRequest {}
impl crate::request::sealed::Sealed for ModifyGttOrderRequest {}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct GttOrderId(String);
impl GttOrderId {
    pub fn new(value: impl Into<String>) -> Result<Self, ValidationError> {
        let value = value.into();
        if value.trim().is_empty() || value.len() > 128 {
            Err(ValidationError::new("GTT order ID is invalid"))
        } else {
            Ok(Self(value))
        }
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl<'de> Deserialize<'de> for GttOrderId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GttLegType {
    Target,
    StopLoss,
}
impl GttLegType {
    fn wire(self) -> &'static str {
        match self {
            Self::Target => "target",
            Self::StopLoss => "stoploss",
        }
    }
}

#[derive(Clone, Debug)]
pub struct GttLeg {
    kind: GttLegType,
    action: Action,
    limit_price: Money,
    trigger_price: Money,
}
impl GttLeg {
    pub fn target(
        action: Action,
        limit_price: Money,
        trigger_price: Money,
    ) -> Result<Self, ValidationError> {
        if action == Action::Sell && limit_price.decimal() < trigger_price.decimal() {
            return Err(ValidationError::new(
                "sell target limit must not be below its trigger",
            ));
        }
        Ok(Self {
            kind: GttLegType::Target,
            action,
            limit_price,
            trigger_price,
        })
    }
    pub fn stop_loss(
        action: Action,
        limit_price: Money,
        trigger_price: Money,
    ) -> Result<Self, ValidationError> {
        if action == Action::Sell && limit_price.decimal() > trigger_price.decimal() {
            return Err(ValidationError::new(
                "sell stop-loss limit must not exceed its trigger",
            ));
        }
        Ok(Self {
            kind: GttLegType::StopLoss,
            action,
            limit_price,
            trigger_price,
        })
    }
}

#[derive(Clone, Debug)]
pub struct GttLegSet {
    target: GttLeg,
    stop_loss: GttLeg,
}
impl GttLegSet {
    pub fn cover_oco(first: GttLeg, second: GttLeg) -> Result<Self, ValidationError> {
        match (first.kind, second.kind) {
            (GttLegType::Target, GttLegType::StopLoss) => Ok(Self {
                target: first,
                stop_loss: second,
            }),
            (GttLegType::StopLoss, GttLegType::Target) => Ok(Self {
                target: second,
                stop_loss: first,
            }),
            _ => Err(ValidationError::new(
                "cover OCO requires one target and one stop-loss leg",
            )),
        }
    }

    fn iter(&self) -> impl Iterator<Item = &GttLeg> {
        [&self.target, &self.stop_loss].into_iter()
    }
}

#[derive(Clone, Debug)]
pub struct FreshGttOrder {
    action: Action,
    price: Money,
}
impl FreshGttOrder {
    pub fn limit(action: Action, price: Money) -> Self {
        Self { action, price }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GttIndexKind {
    Index,
    Stock,
}
impl GttIndexKind {
    fn wire(self) -> &'static str {
        match self {
            Self::Index => "index",
            Self::Stock => "stock",
        }
    }
}

#[derive(Clone, Debug)]
pub struct GttOrderRequest {
    instrument: Instrument,
    quantity: Quantity,
    kind: GttOrderKind,
    index_kind: Option<GttIndexKind>,
    trade_date: Option<NaiveDate>,
}

#[derive(Clone, Debug)]
enum GttOrderKind {
    Single(GttLeg),
    CoverOco {
        fresh: FreshGttOrder,
        legs: GttLegSet,
    },
}

#[derive(Clone, Debug)]
pub struct GttOrderBuilder(GttOrderRequest);
impl GttOrderRequest {
    /// Creates the documented single-leg GTT request.
    pub fn single(instrument: Instrument, quantity: Quantity, leg: GttLeg) -> GttOrderBuilder {
        GttOrderBuilder(Self {
            instrument,
            quantity,
            kind: GttOrderKind::Single(leg),
            index_kind: None,
            trade_date: None,
        })
    }

    /// Creates the documented fresh-order plus target/stop-loss GTT request.
    pub fn cover_oco(
        instrument: Instrument,
        quantity: Quantity,
        fresh: FreshGttOrder,
        legs: GttLegSet,
    ) -> GttOrderBuilder {
        GttOrderBuilder(Self {
            instrument,
            quantity,
            kind: GttOrderKind::CoverOco { fresh, legs },
            index_kind: None,
            trade_date: None,
        })
    }
}
impl GttOrderBuilder {
    pub fn index_kind(mut self, value: GttIndexKind) -> Self {
        self.0.index_kind = Some(value);
        self
    }
    pub fn trade_date(mut self, value: NaiveDate) -> Self {
        self.0.trade_date = Some(value);
        self
    }
    pub fn build(self) -> Result<GttOrderRequest, ValidationError> {
        if self.0.instrument.expiry().is_none() {
            return Err(ValidationError::new(
                "GTT orders require a derivative instrument",
            ));
        }
        if matches!(&self.0.kind, GttOrderKind::CoverOco { .. })
            && self.0.instrument.right().is_none()
        {
            return Err(ValidationError::new(
                "cover OCO requires an option instrument",
            ));
        }
        if self.0.index_kind.is_none() || self.0.trade_date.is_none() {
            return Err(ValidationError::new(
                "index kind and trade date are required",
            ));
        }
        Ok(self.0)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct GttReceipt {
    gtt_order_id: GttOrderId,
    message: String,
}
impl GttReceipt {
    pub fn gtt_order_id(&self) -> &GttOrderId {
        &self.gtt_order_id
    }
    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Serialize)]
struct WireLeg<'a> {
    gtt_leg_type: &'a str,
    action: &'a str,
    limit_price: String,
    trigger_price: String,
}
fn wire_legs(legs: &GttLegSet) -> Vec<WireLeg<'_>> {
    legs.iter().map(wire_leg).collect()
}

fn wire_leg(leg: &GttLeg) -> WireLeg<'_> {
    WireLeg {
        gtt_leg_type: leg.kind.wire(),
        action: leg.action.wire(),
        limit_price: leg.limit_price.to_wire_string(),
        trigger_price: leg.trigger_price.to_wire_string(),
    }
}

impl EndpointRequest for GttOrderRequest {
    type Response = GttReceipt;
    fn operation(&self) -> &'static str {
        "gttorder"
    }
    fn method(&self) -> Method {
        Method::POST
    }
    fn path(&self) -> &'static str {
        "/gttorder"
    }
    fn request_class(&self) -> RequestClass {
        RequestClass::GttMutation
    }
    fn body(&self) -> Result<Vec<u8>, Error> {
        match &self.kind {
            GttOrderKind::Single(leg) => {
                #[derive(Serialize)]
                struct Body<'a> {
                    exchange_code: &'a str,
                    stock_code: &'a str,
                    product: &'a str,
                    quantity: String,
                    expiry_date: String,
                    right: &'a str,
                    strike_price: String,
                    gtt_type: &'a str,
                    index_or_stock: &'a str,
                    trade_date: String,
                    order_details: Vec<WireLeg<'a>>,
                }
                compact_json(&Body {
                    exchange_code: self.instrument.exchange_wire(),
                    stock_code: self.instrument.stock_code().as_str(),
                    product: self.instrument.product().wire(),
                    quantity: self.quantity.get().to_string(),
                    expiry_date: expiry_timestamp(
                        self.instrument.expiry().expect("validated derivative"),
                    ),
                    right: self
                        .instrument
                        .right()
                        .map_or("others", |right| right.wire()),
                    strike_price: self
                        .instrument
                        .strike()
                        .map_or_else(|| "0".to_owned(), Money::to_wire_string),
                    gtt_type: "single",
                    index_or_stock: self.index_kind.expect("validated index kind").wire(),
                    trade_date: expiry_timestamp(self.trade_date.expect("validated trade date")),
                    order_details: vec![wire_leg(leg)],
                })
            }
            GttOrderKind::CoverOco { fresh, legs } => {
                #[derive(Serialize)]
                struct Body<'a> {
                    exchange_code: &'a str,
                    stock_code: &'a str,
                    product: &'a str,
                    quantity: String,
                    expiry_date: String,
                    right: &'a str,
                    strike_price: String,
                    gtt_type: &'a str,
                    fresh_order_action: &'a str,
                    fresh_order_price: String,
                    fresh_order_type: &'a str,
                    index_or_stock: &'a str,
                    trade_date: String,
                    order_details: Vec<WireLeg<'a>>,
                }
                compact_json(&Body {
                    exchange_code: self.instrument.exchange_wire(),
                    stock_code: self.instrument.stock_code().as_str(),
                    product: self.instrument.product().wire(),
                    quantity: self.quantity.get().to_string(),
                    expiry_date: expiry_timestamp(
                        self.instrument.expiry().expect("validated option"),
                    ),
                    right: self.instrument.right().expect("validated option").wire(),
                    strike_price: self
                        .instrument
                        .strike()
                        .expect("validated option")
                        .to_wire_string(),
                    gtt_type: "cover_oco",
                    fresh_order_action: fresh.action.wire(),
                    fresh_order_price: fresh.price.to_wire_string(),
                    fresh_order_type: "limit",
                    index_or_stock: self.index_kind.expect("validated index kind").wire(),
                    trade_date: expiry_timestamp(self.trade_date.expect("validated trade date")),
                    order_details: wire_legs(legs),
                })
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct GttOrderListRequest {
    range: DateRange,
}
impl GttOrderListRequest {
    pub fn new(range: DateRange) -> Result<Self, ValidationError> {
        Ok(Self { range })
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct GttOrder {
    #[serde(default)]
    order_details: Vec<Value>,
    exchange_code: String,
    stock_code: String,
    #[serde(default)]
    fresh_order_id: Option<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}
impl GttOrder {
    pub fn order_details(&self) -> &[Value] {
        &self.order_details
    }
    pub fn exchange_code(&self) -> &str {
        &self.exchange_code
    }
    pub fn stock_code(&self) -> &str {
        &self.stock_code
    }
    pub fn fresh_order_id(&self) -> Option<&str> {
        self.fresh_order_id.as_deref()
    }
    pub fn extra(&self) -> &BTreeMap<String, Value> {
        &self.extra
    }
}

impl EndpointRequest for GttOrderListRequest {
    type Response = Vec<GttOrder>;
    fn operation(&self) -> &'static str {
        "gttorder"
    }
    fn method(&self) -> Method {
        Method::GET
    }
    fn path(&self) -> &'static str {
        "/gttorder"
    }
    fn body(&self) -> Result<Vec<u8>, Error> {
        #[derive(Serialize)]
        struct Body<'a> {
            exchange_code: &'a str,
            from_date: String,
            to_date: String,
        }
        compact_json(&Body {
            exchange_code: "NFO",
            from_date: self.range.start_wire(),
            to_date: self.range.end_wire(),
        })
    }
}

#[derive(Clone, Debug)]
pub struct CancelGttOrderRequest {
    order_id: GttOrderId,
}
impl CancelGttOrderRequest {
    pub fn new(order_id: GttOrderId) -> Self {
        Self { order_id }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct CancelGttReceipt {
    #[serde(rename = "order_id")]
    gtt_order_id: GttOrderId,
    message: String,
}
impl CancelGttReceipt {
    pub fn gtt_order_id(&self) -> &GttOrderId {
        &self.gtt_order_id
    }
    pub fn message(&self) -> &str {
        &self.message
    }
}
impl EndpointRequest for CancelGttOrderRequest {
    type Response = CancelGttReceipt;
    fn operation(&self) -> &'static str {
        "gttorder"
    }
    fn method(&self) -> Method {
        Method::DELETE
    }
    fn path(&self) -> &'static str {
        "/gttorder"
    }
    fn request_class(&self) -> RequestClass {
        RequestClass::GttMutation
    }
    fn body(&self) -> Result<Vec<u8>, Error> {
        #[derive(Serialize)]
        struct Body<'a> {
            gtt_order_id: &'a str,
            exchange_code: &'a str,
        }
        compact_json(&Body {
            gtt_order_id: self.order_id.as_str(),
            exchange_code: "NFO",
        })
    }
}

#[derive(Clone, Debug)]
pub struct ModifyGttOrderRequest {
    order_id: GttOrderId,
    kind: ModifyGttOrderKind,
}

#[derive(Clone, Debug)]
enum ModifyGttOrderKind {
    Single(GttLeg),
    CoverOco(GttLegSet),
}
impl ModifyGttOrderRequest {
    pub fn single(order_id: GttOrderId, leg: GttLeg) -> Self {
        Self {
            order_id,
            kind: ModifyGttOrderKind::Single(leg),
        }
    }

    pub fn cover_oco(order_id: GttOrderId, legs: GttLegSet) -> Self {
        Self {
            order_id,
            kind: ModifyGttOrderKind::CoverOco(legs),
        }
    }
}
impl EndpointRequest for ModifyGttOrderRequest {
    type Response = GttReceipt;
    fn operation(&self) -> &'static str {
        "gttorder"
    }
    fn method(&self) -> Method {
        Method::PUT
    }
    fn path(&self) -> &'static str {
        "/gttorder"
    }
    fn request_class(&self) -> RequestClass {
        RequestClass::GttMutation
    }
    fn body(&self) -> Result<Vec<u8>, Error> {
        #[derive(Serialize)]
        struct Body<'a> {
            exchange_code: &'a str,
            gtt_order_id: &'a str,
            gtt_type: &'a str,
            order_details: Vec<WireLeg<'a>>,
        }
        let (gtt_type, order_details) = match &self.kind {
            ModifyGttOrderKind::Single(leg) => ("single", vec![wire_leg(leg)]),
            ModifyGttOrderKind::CoverOco(legs) => ("cover_oco", wire_legs(legs)),
        };
        compact_json(&Body {
            exchange_code: "NFO",
            gtt_order_id: self.order_id.as_str(),
            gtt_type,
            order_details,
        })
    }
}
