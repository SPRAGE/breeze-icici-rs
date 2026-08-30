use std::fmt;
use std::num::NonZeroU64;
use std::str::FromStr;

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer};

use crate::error::ValidationError;

/// An upstream string value not recognized by this crate version.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct UnknownValue(String);

impl UnknownValue {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A normalized broker stock code.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StockCode(String);

impl StockCode {
    pub fn new(value: impl AsRef<str>) -> Result<Self, ValidationError> {
        let value = value.as_ref().trim().to_ascii_uppercase();
        if value.is_empty()
            || value.len() > 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"&._-".contains(&byte))
        {
            return Err(ValidationError::new("invalid stock code"));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StockCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for StockCode {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(de::Error::custom)
    }
}

/// A decimal value parsed without binary floating-point conversion.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Money(Decimal);

impl Money {
    pub fn to_wire_string(&self) -> String {
        self.0.to_string()
    }

    pub fn decimal(&self) -> Decimal {
        self.0
    }
}

impl fmt::Display for Money {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
    }
}

impl FromStr for Money {
    type Err = ValidationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let valid = !value.is_empty()
            && !value.contains(['e', 'E', ','])
            && value.bytes().enumerate().all(|(index, byte)| {
                byte.is_ascii_digit() || byte == b'.' || (byte == b'-' && index == 0)
            })
            && value.bytes().filter(|byte| *byte == b'.').count() <= 1
            && value.bytes().any(|byte| byte.is_ascii_digit());
        if !valid {
            return Err(ValidationError::new("invalid decimal value"));
        }
        Decimal::from_str_exact(value)
            .map(Self)
            .map_err(|_| ValidationError::new("invalid decimal value"))
    }
}

impl<'de> Deserialize<'de> for Money {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct MoneyVisitor;
        impl<'de> Visitor<'de> for MoneyVisitor {
            type Value = Money;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a decimal string or JSON number")
            }
            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Money::from_str(value).map_err(E::custom)
            }
            fn visit_string<E: de::Error>(self, value: String) -> Result<Self::Value, E> {
                self.visit_str(&value)
            }
            fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
                Money::from_str(&value.to_string()).map_err(E::custom)
            }
            fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
                Money::from_str(&value.to_string()).map_err(E::custom)
            }
            fn visit_f64<E: de::Error>(self, value: f64) -> Result<Self::Value, E> {
                if !value.is_finite() {
                    return Err(E::custom("non-finite decimal"));
                }
                let value = if value.fract() == 0.0 {
                    format!("{value:.1}")
                } else {
                    value.to_string()
                };
                Money::from_str(&value).map_err(E::custom)
            }
        }
        deserializer.deserialize_any(MoneyVisitor)
    }
}

/// A strictly positive order quantity.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Quantity(NonZeroU64);

impl Quantity {
    pub fn new(value: u64) -> Result<Self, ValidationError> {
        NonZeroU64::new(value)
            .map(Self)
            .ok_or_else(|| ValidationError::new("quantity must be positive"))
    }

    pub fn get(self) -> u64 {
        self.0.get()
    }
}

impl<'de> Deserialize<'de> for Quantity {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Wire {
            Number(u64),
            Text(String),
        }
        let value = match Wire::deserialize(deserializer)? {
            Wire::Number(value) => value,
            Wire::Text(value) => value.parse().map_err(de::Error::custom)?,
        };
        Self::new(value).map_err(de::Error::custom)
    }
}

/// A non-negative count used for market volume and depth quantities.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Count(u64);

impl Count {
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

impl<'de> Deserialize<'de> for Count {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Wire {
            Number(u64),
            Text(String),
        }
        match Wire::deserialize(deserializer)? {
            Wire::Number(value) => Ok(Self(value)),
            Wire::Text(value) => value.parse().map(Self).map_err(de::Error::custom),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum Exchange {
    Nse,
    Bse,
    Nfo,
    Bfo,
    Ndx,
    Mcx,
}

impl Exchange {
    pub(crate) fn wire(self) -> &'static str {
        match self {
            Self::Nse => "NSE",
            Self::Bse => "BSE",
            Self::Nfo => "NFO",
            Self::Bfo => "BFO",
            Self::Ndx => "NDX",
            Self::Mcx => "MCX",
        }
    }

    pub(crate) fn from_wire(value: &str) -> Option<Self> {
        match value.to_ascii_uppercase().as_str() {
            "NSE" => Some(Self::Nse),
            "BSE" => Some(Self::Bse),
            "NFO" => Some(Self::Nfo),
            "BFO" => Some(Self::Bfo),
            "NDX" | "CDNSE" => Some(Self::Ndx),
            "MCX" => Some(Self::Mcx),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DerivativeExchange {
    Nfo,
    Bfo,
    Ndx,
    Mcx,
}

impl DerivativeExchange {
    pub(crate) fn wire(self) -> &'static str {
        match self {
            Self::Nfo => "NFO",
            Self::Bfo => "BFO",
            Self::Ndx => "NDX",
            Self::Mcx => "MCX",
        }
    }

    pub(crate) fn exchange(self) -> Exchange {
        match self {
            Self::Nfo => Exchange::Nfo,
            Self::Bfo => Exchange::Bfo,
            Self::Ndx => Exchange::Ndx,
            Self::Mcx => Exchange::Mcx,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum OptionRight {
    Call,
    Put,
}

impl OptionRight {
    pub(crate) fn wire(self) -> &'static str {
        match self {
            Self::Call => "call",
            Self::Put => "put",
        }
    }

    pub(crate) fn from_wire(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "call" | "ce" | "c" => Some(Self::Call),
            "put" | "pe" | "p" => Some(Self::Put),
            _ => None,
        }
    }
}

impl<'de> Deserialize<'de> for OptionRight {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::from_wire(&value).ok_or_else(|| de::Error::custom("unknown option right"))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Product {
    Cash,
    Futures,
    Options,
}

impl Product {
    pub(crate) fn wire(self) -> &'static str {
        match self {
            Self::Cash => "cash",
            Self::Futures => "futures",
            Self::Options => "options",
        }
    }
}

impl FromStr for Product {
    type Err = ValidationError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "cash" => Ok(Self::Cash),
            "future" | "futures" => Ok(Self::Futures),
            "option" | "options" => Ok(Self::Options),
            _ => Err(ValidationError::new("unsupported product")),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Interval {
    OneMinute,
    OneDay,
}

impl Interval {
    pub(crate) fn v1_wire(self) -> &'static str {
        match self {
            Self::OneMinute => "minute",
            Self::OneDay => "day",
        }
    }

    pub(crate) fn v2_wire(self) -> &'static str {
        match self {
            Self::OneMinute => "1minute",
            Self::OneDay => "1day",
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct UserRemark(String);

impl UserRemark {
    pub fn new(value: impl AsRef<str>) -> Result<Self, ValidationError> {
        let value = value.as_ref();
        if value.is_empty()
            || value.len() > 64
            || !value.bytes().all(|byte| byte.is_ascii_alphanumeric())
        {
            return Err(ValidationError::new(
                "remark must contain only ASCII letters and digits",
            ));
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DateRange {
    from: DateTime<Utc>,
    to: DateTime<Utc>,
}

impl DateRange {
    pub fn new(from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Self, ValidationError> {
        if from > to {
            return Err(ValidationError::new("from date must not be after to date"));
        }
        Ok(Self { from, to })
    }

    pub fn from(&self) -> DateTime<Utc> {
        self.from
    }

    pub fn to(&self) -> DateTime<Utc> {
        self.to
    }

    pub(crate) fn start_wire(&self) -> String {
        timestamp(self.from)
    }

    pub(crate) fn end_wire(&self) -> String {
        timestamp(self.to)
    }
}

fn timestamp(value: DateTime<Utc>) -> String {
    value.format("%Y-%m-%dT%H:%M:%S.000Z").to_string()
}

pub(crate) fn expiry_timestamp(value: NaiveDate) -> String {
    value.format("%Y-%m-%dT00:00:00.000Z").to_string()
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum InstrumentKind {
    Equity {
        exchange: Exchange,
    },
    Future {
        exchange: DerivativeExchange,
        expiry: NaiveDate,
    },
    Option {
        exchange: DerivativeExchange,
        expiry: NaiveDate,
        right: OptionRight,
        strike: Money,
    },
}

/// Full cash, future, or option identity. Variant-specific fields cannot be
/// attached to an incompatible instrument.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Instrument {
    stock_code: StockCode,
    kind: InstrumentKind,
}

impl Instrument {
    pub fn equity(exchange: Exchange, stock_code: StockCode) -> Result<Self, ValidationError> {
        if !matches!(exchange, Exchange::Nse | Exchange::Bse) {
            return Err(ValidationError::new("equity requires NSE or BSE"));
        }
        Ok(Self {
            stock_code,
            kind: InstrumentKind::Equity { exchange },
        })
    }

    pub fn future(
        exchange: DerivativeExchange,
        stock_code: StockCode,
        expiry: NaiveDate,
    ) -> Result<Self, ValidationError> {
        Ok(Self {
            stock_code,
            kind: InstrumentKind::Future { exchange, expiry },
        })
    }

    pub fn option(
        exchange: DerivativeExchange,
        stock_code: StockCode,
        expiry: NaiveDate,
        right: OptionRight,
        strike: Money,
    ) -> Result<Self, ValidationError> {
        Ok(Self {
            stock_code,
            kind: InstrumentKind::Option {
                exchange,
                expiry,
                right,
                strike,
            },
        })
    }

    pub fn stock_code(&self) -> &StockCode {
        &self.stock_code
    }

    pub fn expiry(&self) -> Option<NaiveDate> {
        match self.kind {
            InstrumentKind::Equity { .. } => None,
            InstrumentKind::Future { expiry, .. } | InstrumentKind::Option { expiry, .. } => {
                Some(expiry)
            }
        }
    }

    pub fn right(&self) -> Option<OptionRight> {
        match self.kind {
            InstrumentKind::Option { right, .. } => Some(right),
            _ => None,
        }
    }

    pub fn strike(&self) -> Option<&Money> {
        match &self.kind {
            InstrumentKind::Option { strike, .. } => Some(strike),
            _ => None,
        }
    }

    pub fn product(&self) -> Product {
        match self.kind {
            InstrumentKind::Equity { .. } => Product::Cash,
            InstrumentKind::Future { .. } => Product::Futures,
            InstrumentKind::Option { .. } => Product::Options,
        }
    }

    pub fn exchange(&self) -> Exchange {
        match self.kind {
            InstrumentKind::Equity { exchange } => exchange,
            InstrumentKind::Future { exchange, .. } | InstrumentKind::Option { exchange, .. } => {
                exchange.exchange()
            }
        }
    }

    pub(crate) fn exchange_wire(&self) -> &'static str {
        self.exchange().wire()
    }
}
