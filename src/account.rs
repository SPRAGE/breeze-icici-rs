use std::collections::BTreeMap;
use std::fmt;

use http::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::EndpointRequest;
use crate::auth::{ApiSession, AppKey, SessionToken};
use crate::domain::{Exchange, Money, Quantity, StockCode};
use crate::error::{Error, ValidationError};
use crate::rate_limit::RequestClass;
use crate::request::{AuthenticationMode, compact_json};

impl crate::request::sealed::Sealed for CustomerDetailsRequest {}
impl crate::request::sealed::Sealed for GetDematHoldings {}
impl crate::request::sealed::Sealed for GetFunds {}
impl crate::request::sealed::Sealed for SetFundsRequest {}
impl crate::request::sealed::Sealed for GetMarginRequest {}

#[derive(Clone, Debug)]
pub struct CustomerDetailsRequest {
    app_key: AppKey,
    api_session: ApiSession,
}

impl CustomerDetailsRequest {
    pub fn new(app_key: AppKey, api_session: ApiSession) -> Self {
        Self {
            app_key,
            api_session,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(transparent)]
pub struct UserId(String);

impl UserId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Deserialize)]
pub struct CustomerDetails {
    #[serde(rename = "idirect_userid")]
    user_id: UserId,
    #[serde(default, deserialize_with = "optional_session_token")]
    session_token: Option<SessionToken>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

fn optional_session_token<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<SessionToken>, D::Error> {
    let value = Option::<String>::deserialize(deserializer)?;
    value
        .map(SessionToken::new)
        .transpose()
        .map_err(serde::de::Error::custom)
}

impl fmt::Debug for CustomerDetails {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CustomerDetails")
            .field("user_id", &self.user_id)
            .field(
                "session_token",
                &self.session_token.as_ref().map(|_| "[REDACTED]"),
            )
            .field("extra_fields", &self.extra.len())
            .finish()
    }
}

impl CustomerDetails {
    pub fn user_id(&self) -> &UserId {
        &self.user_id
    }
    pub fn session_token(&self) -> Option<&SessionToken> {
        self.session_token.as_ref()
    }
}

impl EndpointRequest for CustomerDetailsRequest {
    type Response = CustomerDetails;
    fn operation(&self) -> &'static str {
        "customerdetails"
    }
    fn method(&self) -> Method {
        Method::GET
    }
    fn path(&self) -> &'static str {
        "/customerdetails"
    }
    fn authentication(&self) -> AuthenticationMode {
        AuthenticationMode::SessionExchange
    }
    fn body(&self) -> Result<Vec<u8>, Error> {
        #[derive(Serialize)]
        #[serde(rename_all = "PascalCase")]
        struct Body<'a> {
            session_token: &'a str,
            app_key: &'a str,
        }
        compact_json(&Body {
            session_token: self.api_session.expose(),
            app_key: self.app_key.expose(),
        })
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct GetDematHoldings;

#[derive(Clone, Debug, Deserialize)]
pub struct DematHolding {
    stock_code: StockCode,
    #[serde(default)]
    stock_isin: Option<String>,
    quantity: Quantity,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

impl DematHolding {
    pub fn stock_code(&self) -> &StockCode {
        &self.stock_code
    }
    pub fn quantity(&self) -> Quantity {
        self.quantity
    }
    pub fn isin(&self) -> Option<&str> {
        self.stock_isin.as_deref()
    }
    pub fn extra(&self) -> &BTreeMap<String, Value> {
        &self.extra
    }
}

impl EndpointRequest for GetDematHoldings {
    type Response = Vec<DematHolding>;
    fn operation(&self) -> &'static str {
        "dematholdings"
    }
    fn method(&self) -> Method {
        Method::GET
    }
    fn path(&self) -> &'static str {
        "/dematholdings"
    }
    fn body(&self) -> Result<Vec<u8>, Error> {
        Ok(b"{}".to_vec())
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct GetFunds;

#[derive(Clone, Deserialize)]
pub struct Funds {
    #[serde(default)]
    bank_account: Option<String>,
    total_bank_balance: Money,
    allocated_equity: Money,
    allocated_fno: Money,
    allocated_commodity: Money,
    allocated_currency: Money,
    block_by_trade_equity: Money,
    block_by_trade_fno: Money,
    block_by_trade_commodity: Money,
    block_by_trade_currency: Money,
    block_by_trade_balance: Money,
    unallocated_balance: Money,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

impl fmt::Debug for Funds {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Funds")
            .field(
                "bank_account",
                &self.bank_account.as_ref().map(|_| "[REDACTED]"),
            )
            .field("total_bank_balance", &self.total_bank_balance)
            .field("unallocated_balance", &self.unallocated_balance)
            .finish_non_exhaustive()
    }
}

impl Funds {
    pub fn total_bank_balance(&self) -> &Money {
        &self.total_bank_balance
    }
    pub fn unallocated_balance(&self) -> &Money {
        &self.unallocated_balance
    }
    pub fn allocated_equity(&self) -> &Money {
        &self.allocated_equity
    }
    pub fn allocated_fno(&self) -> &Money {
        &self.allocated_fno
    }
    pub fn allocated_commodity(&self) -> &Money {
        &self.allocated_commodity
    }
    pub fn allocated_currency(&self) -> &Money {
        &self.allocated_currency
    }
    pub fn block_by_trade_equity(&self) -> &Money {
        &self.block_by_trade_equity
    }
    pub fn block_by_trade_fno(&self) -> &Money {
        &self.block_by_trade_fno
    }
    pub fn block_by_trade_commodity(&self) -> &Money {
        &self.block_by_trade_commodity
    }
    pub fn block_by_trade_currency(&self) -> &Money {
        &self.block_by_trade_currency
    }
    pub fn block_by_trade_balance(&self) -> &Money {
        &self.block_by_trade_balance
    }
    pub fn extra(&self) -> &BTreeMap<String, Value> {
        &self.extra
    }
}

impl EndpointRequest for GetFunds {
    type Response = Funds;
    fn operation(&self) -> &'static str {
        "funds"
    }
    fn method(&self) -> Method {
        Method::GET
    }
    fn path(&self) -> &'static str {
        "/funds"
    }
    fn body(&self) -> Result<Vec<u8>, Error> {
        Ok(b"{}".to_vec())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FundTransaction {
    Credit,
    Debit,
}
impl FundTransaction {
    fn wire(self) -> &'static str {
        match self {
            Self::Credit => "Credit",
            Self::Debit => "Debit",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FundSegment {
    Equity,
    FuturesAndOptions,
    Commodity,
    Currency,
}
impl FundSegment {
    fn wire(self) -> &'static str {
        match self {
            Self::Equity => "Equity",
            Self::FuturesAndOptions => "FNO",
            Self::Commodity => "Commodity",
            Self::Currency => "Currency",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FundAmount(u64);
impl FundAmount {
    pub fn new(value: u64) -> Result<Self, ValidationError> {
        if value == 0 {
            Err(ValidationError::new("fund amount must be positive"))
        } else {
            Ok(Self(value))
        }
    }
    pub fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug)]
pub struct SetFundsRequest {
    transaction: FundTransaction,
    amount: FundAmount,
    segment: FundSegment,
}
impl SetFundsRequest {
    pub fn new(transaction: FundTransaction, amount: FundAmount, segment: FundSegment) -> Self {
        Self {
            transaction,
            amount,
            segment,
        }
    }
    pub fn transaction(&self) -> FundTransaction {
        self.transaction
    }
    pub fn segment(&self) -> FundSegment {
        self.segment
    }
    pub fn amount(&self) -> FundAmount {
        self.amount
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct SetFundsReceipt {
    status: String,
}
impl SetFundsReceipt {
    pub fn status(&self) -> &str {
        &self.status
    }
}

impl EndpointRequest for SetFundsRequest {
    type Response = SetFundsReceipt;
    fn operation(&self) -> &'static str {
        "funds"
    }
    fn method(&self) -> Method {
        Method::POST
    }
    fn path(&self) -> &'static str {
        "/funds"
    }
    fn request_class(&self) -> RequestClass {
        RequestClass::SetFunds
    }
    fn body(&self) -> Result<Vec<u8>, Error> {
        #[derive(Serialize)]
        struct Body<'a> {
            transaction_type: &'a str,
            amount: String,
            segment: &'a str,
        }
        compact_json(&Body {
            transaction_type: self.transaction.wire(),
            amount: self.amount.0.to_string(),
            segment: self.segment.wire(),
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GetMarginRequest {
    exchange: Exchange,
}
impl GetMarginRequest {
    pub fn new(exchange: Exchange) -> Self {
        Self { exchange }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Margin {
    #[serde(default)]
    limit_list: Vec<Value>,
    cash_limit: Money,
    amount_allocated: Money,
    block_by_trade: Money,
    isec_margin: Money,
}

impl Margin {
    pub fn cash_limit(&self) -> &Money {
        &self.cash_limit
    }
    pub fn amount_allocated(&self) -> &Money {
        &self.amount_allocated
    }
    pub fn block_by_trade(&self) -> &Money {
        &self.block_by_trade
    }
    pub fn isec_margin(&self) -> &Money {
        &self.isec_margin
    }
    pub fn limit_list(&self) -> &[Value] {
        &self.limit_list
    }
}

impl EndpointRequest for GetMarginRequest {
    type Response = Margin;
    fn operation(&self) -> &'static str {
        "margin"
    }
    fn method(&self) -> Method {
        Method::GET
    }
    fn path(&self) -> &'static str {
        "/margin"
    }
    fn body(&self) -> Result<Vec<u8>, Error> {
        #[derive(Serialize)]
        struct Body<'a> {
            exchange_code: &'a str,
        }
        compact_json(&Body {
            exchange_code: self.exchange.wire(),
        })
    }
}
