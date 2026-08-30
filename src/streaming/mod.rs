use std::collections::HashSet;
use std::fmt;

use crate::domain::{Count, Exchange, Money, OptionRight, StockCode};
use crate::instruments::ScriptCode;
#[cfg(any(feature = "streaming", feature = "test-util"))]
use crate::instruments::ScriptDataKind;
use crate::orders::OrderId;

pub mod codec;

#[cfg(feature = "test-util")]
pub(crate) mod testing;

#[cfg(feature = "streaming")]
mod production;
#[cfg(feature = "streaming")]
pub use production::StreamingClient;

#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum StreamEvent {
    Quote(QuoteTick),
    MarketDepth(MarketDepth),
    Commodity(CommodityTick),
    Order(OrderNotification),
    OneClickFno(OneClickFno),
    OneClickEquity(OneClickEquity),
    Candle(Candle),
    Unknown(RawStreamFrame),
}

#[derive(Clone, Debug)]
pub struct RawStreamFrame(serde_json::Value);
impl RawStreamFrame {
    pub fn value(&self) -> &serde_json::Value {
        &self.0
    }
}

#[derive(Clone, Debug)]
pub struct QuoteTick {
    symbol: ScriptCode,
    exchange: Exchange,
    last_price: Money,
    total_sell_quantity: Option<Count>,
    open_interest: Option<Money>,
    change_in_open_interest: Option<Money>,
}
impl QuoteTick {
    pub fn symbol(&self) -> &ScriptCode {
        &self.symbol
    }
    pub fn exchange(&self) -> Exchange {
        self.exchange
    }
    pub fn last_price(&self) -> &Money {
        &self.last_price
    }
    pub fn total_sell_quantity(&self) -> Option<Count> {
        self.total_sell_quantity
    }
    pub fn open_interest(&self) -> Option<&Money> {
        self.open_interest.as_ref()
    }
    pub fn change_in_open_interest(&self) -> Option<&Money> {
        self.change_in_open_interest.as_ref()
    }
}

#[derive(Clone, Debug)]
pub struct DepthLevel {
    buy_price: Money,
    buy_quantity: Count,
    buy_orders: Option<u64>,
    sell_price: Money,
    sell_quantity: Count,
    sell_orders: Option<u64>,
}
impl DepthLevel {
    pub fn buy_price(&self) -> &Money {
        &self.buy_price
    }
    pub fn buy_quantity(&self) -> Count {
        self.buy_quantity
    }
    pub fn buy_orders(&self) -> Option<u64> {
        self.buy_orders
    }
    pub fn sell_price(&self) -> &Money {
        &self.sell_price
    }
    pub fn sell_quantity(&self) -> Count {
        self.sell_quantity
    }
    pub fn sell_orders(&self) -> Option<u64> {
        self.sell_orders
    }
}

#[derive(Clone, Debug)]
pub struct MarketDepth {
    symbol: ScriptCode,
    exchange: Exchange,
    levels: Vec<DepthLevel>,
}
impl MarketDepth {
    pub fn symbol(&self) -> &ScriptCode {
        &self.symbol
    }
    pub fn exchange(&self) -> Exchange {
        self.exchange
    }
    pub fn levels(&self) -> &[DepthLevel] {
        &self.levels
    }
}

#[derive(Clone, Debug)]
pub struct CommodityTick {
    symbol: ScriptCode,
    last_price: Money,
    current_open_interest: Money,
    depth: Vec<DepthLevel>,
}
impl CommodityTick {
    pub fn symbol(&self) -> &ScriptCode {
        &self.symbol
    }
    pub fn last_price(&self) -> &Money {
        &self.last_price
    }
    pub fn current_open_interest(&self) -> &Money {
        &self.current_open_interest
    }
    pub fn depth(&self) -> &[DepthLevel] {
        &self.depth
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KnownText {
    raw: String,
    known: Option<&'static str>,
}
impl KnownText {
    fn known(raw: impl Into<String>, known: &'static str) -> Self {
        Self {
            raw: raw.into(),
            known: Some(known),
        }
    }
    fn unknown(raw: impl Into<String>) -> Self {
        Self {
            raw: raw.into(),
            known: None,
        }
    }
    pub fn as_known_str(&self) -> Option<&'static str> {
        self.known
    }
    pub fn as_str(&self) -> &str {
        &self.raw
    }
}

#[derive(Clone, Debug)]
pub struct OrderNotification {
    stock_code: StockCode,
    action: KnownText,
    status: KnownText,
    order_reference: OrderId,
    right: Option<OptionRight>,
    strike: Option<Money>,
}
impl OrderNotification {
    pub fn stock_code(&self) -> &StockCode {
        &self.stock_code
    }
    pub fn action(&self) -> &KnownText {
        &self.action
    }
    pub fn status(&self) -> &KnownText {
        &self.status
    }
    pub fn order_reference(&self) -> &OrderId {
        &self.order_reference
    }
    pub fn right(&self) -> Option<&OptionRight> {
        self.right.as_ref()
    }
    pub fn strike(&self) -> Option<&Money> {
        self.strike.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextValue(String);
impl TextValue {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug)]
pub struct OneClickFno {
    portfolio_id: TextValue,
    underlying: StockCode,
    status: TextValue,
}
impl OneClickFno {
    pub fn portfolio_id(&self) -> &TextValue {
        &self.portfolio_id
    }
    pub fn underlying(&self) -> &StockCode {
        &self.underlying
    }
    pub fn status(&self) -> &TextValue {
        &self.status
    }
}

#[derive(Clone, Debug)]
pub struct OneClickEquity {
    stock_code: StockCode,
    subscription_type: TextValue,
    status: TextValue,
}
impl OneClickEquity {
    pub fn stock_code(&self) -> &StockCode {
        &self.stock_code
    }
    pub fn subscription_type(&self) -> &TextValue {
        &self.subscription_type
    }
    pub fn status(&self) -> &TextValue {
        &self.status
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CandleInterval {
    OneSecond,
    OneMinute,
    FiveMinutes,
    ThirtyMinutes,
}
impl CandleInterval {
    pub fn from_channel(value: &str) -> Result<Self, StreamDecodeError> {
        match value.to_ascii_uppercase().as_str() {
            "1SEC" => Ok(Self::OneSecond),
            "1MIN" => Ok(Self::OneMinute),
            "5MIN" => Ok(Self::FiveMinutes),
            "30MIN" => Ok(Self::ThirtyMinutes),
            _ => Err(StreamDecodeError::new("unknown candle interval")),
        }
    }
    pub fn channel(self) -> &'static str {
        match self {
            Self::OneSecond => "1SEC",
            Self::OneMinute => "1MIN",
            Self::FiveMinutes => "5MIN",
            Self::ThirtyMinutes => "30MIN",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Candle {
    exchange: Exchange,
    stock_code: StockCode,
    interval: CandleInterval,
    open: Money,
    high: Money,
    low: Money,
    close: Money,
    volume: Count,
    right: Option<OptionRight>,
    strike: Option<Money>,
    open_interest: Option<Money>,
}
impl Candle {
    pub fn exchange(&self) -> Exchange {
        self.exchange
    }
    pub fn stock_code(&self) -> &StockCode {
        &self.stock_code
    }
    pub fn interval(&self) -> CandleInterval {
        self.interval
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
    pub fn right(&self) -> Option<&OptionRight> {
        self.right.as_ref()
    }
    pub fn strike(&self) -> Option<&Money> {
        self.strike.as_ref()
    }
    pub fn open_interest(&self) -> Option<&Money> {
        self.open_interest.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamDecodeError {
    message: String,
}
impl StreamDecodeError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}
impl fmt::Display for StreamDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}
impl std::error::Error for StreamDecodeError {}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum SubscriptionKind {
    Quote,
    MarketDepth,
    Candle(CandleInterval),
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Subscription {
    kind: SubscriptionKind,
    script: ScriptCode,
}
impl Subscription {
    pub fn quote(script: ScriptCode) -> Self {
        Self {
            kind: SubscriptionKind::Quote,
            script,
        }
    }
    pub fn market_depth(script: ScriptCode) -> Self {
        Self {
            kind: SubscriptionKind::MarketDepth,
            script,
        }
    }
    pub fn candle(script: ScriptCode, interval: CandleInterval) -> Self {
        Self {
            kind: SubscriptionKind::Candle(interval),
            script,
        }
    }
    pub fn script(&self) -> &ScriptCode {
        &self.script
    }
    pub fn kind(&self) -> &SubscriptionKind {
        &self.kind
    }

    #[cfg(any(feature = "streaming", feature = "test-util"))]
    pub(crate) fn is_valid_for(&self, stream: StreamKind) -> bool {
        matches!(
            (stream, &self.kind, self.script.data_kind()),
            (
                StreamKind::MarketData,
                SubscriptionKind::Quote,
                ScriptDataKind::Quotes
            ) | (
                StreamKind::MarketData,
                SubscriptionKind::MarketDepth,
                ScriptDataKind::MarketDepth
            ) | (
                StreamKind::Candles,
                SubscriptionKind::Candle(_),
                ScriptDataKind::Quotes
            )
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubscriptionInsert {
    Added,
    AlreadyPresent,
}

#[derive(Clone, Debug)]
pub struct SubscriptionSet {
    values: HashSet<Subscription>,
    limit: usize,
}
impl SubscriptionSet {
    pub fn with_limit(limit: usize) -> Self {
        Self {
            values: HashSet::new(),
            limit,
        }
    }
    pub fn insert(&mut self, value: Subscription) -> Result<SubscriptionInsert, StreamError> {
        if self.values.contains(&value) {
            return Ok(SubscriptionInsert::AlreadyPresent);
        }
        if self.values.len() >= self.limit {
            return Err(StreamError::SubscriptionLimit { limit: self.limit });
        }
        self.values.insert(value);
        Ok(SubscriptionInsert::Added)
    }
    pub fn len(&self) -> usize {
        self.values.len()
    }
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
    pub fn remove(&mut self, value: &Subscription) -> bool {
        self.values.remove(value)
    }
    pub fn contains(&self, value: &Subscription) -> bool {
        self.values.contains(value)
    }
    #[cfg(any(feature = "streaming", feature = "test-util"))]
    pub(crate) fn accepts(&self, stream: StreamKind, value: &Subscription) -> bool {
        if !value.is_valid_for(stream) {
            return false;
        }
        let SubscriptionKind::Candle(candidate) = value.kind() else {
            return true;
        };
        self.values.iter().all(|active| {
            !matches!(active.kind(), SubscriptionKind::Candle(interval) if interval != candidate)
        })
    }
    #[cfg(any(feature = "streaming", feature = "test-util"))]
    pub(crate) fn values(&self) -> impl Iterator<Item = &Subscription> {
        self.values.iter()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamKind {
    MarketData,
    Orders,
    OneClickFno,
    OneClickEquity,
    Candles,
}

#[derive(Clone, Debug, thiserror::Error)]
#[non_exhaustive]
pub enum StreamError {
    #[error("stream consumer lagged by {dropped} event(s); reconcile state through REST")]
    LaggedRequiresReconciliation { dropped: usize },
    #[error("subscription limit of {limit} reached")]
    SubscriptionLimit { limit: usize },
    #[error("stream frame could not be decoded: {0}")]
    Decode(StreamDecodeError),
    #[error("stream is closed")]
    Closed,
    #[error("stream connection failed: {message}")]
    Connection { message: String },
    #[error("subscription is not valid for this stream family")]
    InvalidSubscription,
}

/// A connected, bounded streaming handle. Production connection construction
/// will remain transport-adapter based; the deterministic test adapter exercises
/// the same subscription and overflow semantics.
#[cfg(any(feature = "streaming", feature = "test-util"))]
pub struct StreamHandle {
    pub(crate) inner: StreamHandleInner,
}

#[cfg(any(feature = "streaming", feature = "test-util"))]
pub(crate) enum StreamHandleInner {
    #[cfg(feature = "test-util")]
    Test(testing::TestStreamHandle),
    #[cfg(feature = "streaming")]
    Production(production::ProductionStreamHandle),
}

#[cfg(any(feature = "streaming", feature = "test-util"))]
impl fmt::Debug for StreamHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StreamHandle")
            .finish_non_exhaustive()
    }
}

#[cfg(any(feature = "streaming", feature = "test-util"))]
impl StreamHandle {
    pub async fn subscribe(
        &mut self,
        value: Subscription,
    ) -> Result<SubscriptionInsert, StreamError> {
        match &mut self.inner {
            #[cfg(feature = "test-util")]
            StreamHandleInner::Test(inner) => inner.subscribe(value).await,
            #[cfg(feature = "streaming")]
            StreamHandleInner::Production(inner) => inner.subscribe(value).await,
        }
    }
    pub async fn next_event(&mut self) -> Option<Result<StreamEvent, StreamError>> {
        match &mut self.inner {
            #[cfg(feature = "test-util")]
            StreamHandleInner::Test(inner) => inner.next_event().await,
            #[cfg(feature = "streaming")]
            StreamHandleInner::Production(inner) => inner.next_event().await,
        }
    }
    /// Removes a desired subscription and sends a Socket.IO `leave` event.
    /// Returns `true` when the subscription was active.
    pub async fn unsubscribe(&mut self, value: &Subscription) -> Result<bool, StreamError> {
        match &mut self.inner {
            #[cfg(feature = "test-util")]
            StreamHandleInner::Test(inner) => inner.unsubscribe(value).await,
            #[cfg(feature = "streaming")]
            StreamHandleInner::Production(inner) => inner.unsubscribe(value).await,
        }
    }
    pub async fn wait_until_reconnected(&mut self) -> Result<(), StreamError> {
        match &mut self.inner {
            #[cfg(feature = "test-util")]
            StreamHandleInner::Test(inner) => inner.wait_until_reconnected().await,
            #[cfg(feature = "streaming")]
            StreamHandleInner::Production(inner) => inner.wait_until_reconnected().await,
        }
    }
    /// Waits for reconnect/replay up to `timeout`.
    pub async fn wait_until_reconnected_for(
        &mut self,
        timeout: std::time::Duration,
    ) -> Result<(), StreamError> {
        tokio::time::timeout(timeout, self.wait_until_reconnected())
            .await
            .map_err(|_| StreamError::Connection {
                message: "timed out waiting for stream reconnection".to_owned(),
            })?
    }
    pub async fn shutdown(&mut self) -> Result<(), StreamError> {
        match &mut self.inner {
            #[cfg(feature = "test-util")]
            StreamHandleInner::Test(inner) => inner.shutdown().await,
            #[cfg(feature = "streaming")]
            StreamHandleInner::Production(inner) => inner.shutdown().await,
        }
    }
}

// Codec constructors stay in this module so public fields remain private.
pub(crate) fn quote_tick(
    symbol: ScriptCode,
    exchange: Exchange,
    last_price: Money,
    total_sell_quantity: Option<Count>,
    open_interest: Option<Money>,
    change_in_open_interest: Option<Money>,
) -> QuoteTick {
    QuoteTick {
        symbol,
        exchange,
        last_price,
        total_sell_quantity,
        open_interest,
        change_in_open_interest,
    }
}
pub(crate) fn depth(
    symbol: ScriptCode,
    exchange: Exchange,
    levels: Vec<DepthLevel>,
) -> MarketDepth {
    MarketDepth {
        symbol,
        exchange,
        levels,
    }
}
pub(crate) fn depth_level(
    buy_price: Money,
    buy_quantity: Count,
    buy_orders: Option<u64>,
    sell_price: Money,
    sell_quantity: Count,
    sell_orders: Option<u64>,
) -> DepthLevel {
    DepthLevel {
        buy_price,
        buy_quantity,
        buy_orders,
        sell_price,
        sell_quantity,
        sell_orders,
    }
}
pub(crate) fn commodity(
    symbol: ScriptCode,
    last_price: Money,
    current_open_interest: Money,
    depth: Vec<DepthLevel>,
) -> CommodityTick {
    CommodityTick {
        symbol,
        last_price,
        current_open_interest,
        depth,
    }
}
pub(crate) fn order(
    stock_code: StockCode,
    action: KnownText,
    status: KnownText,
    order_reference: OrderId,
    right: Option<OptionRight>,
    strike: Option<Money>,
) -> OrderNotification {
    OrderNotification {
        stock_code,
        action,
        status,
        order_reference,
        right,
        strike,
    }
}
pub(crate) fn known(raw: &str, value: &'static str) -> KnownText {
    KnownText::known(raw, value)
}
pub(crate) fn unknown(raw: &str) -> KnownText {
    KnownText::unknown(raw)
}
pub(crate) fn one_click_fno(
    portfolio_id: String,
    underlying: StockCode,
    status: String,
) -> OneClickFno {
    OneClickFno {
        portfolio_id: TextValue(portfolio_id),
        underlying,
        status: TextValue(status),
    }
}
pub(crate) fn one_click_equity(
    stock_code: StockCode,
    subscription_type: String,
    status: String,
) -> OneClickEquity {
    OneClickEquity {
        stock_code,
        subscription_type: TextValue(subscription_type),
        status: TextValue(status),
    }
}
pub(crate) struct CandleParts {
    pub exchange: Exchange,
    pub stock_code: StockCode,
    pub interval: CandleInterval,
    pub open: Money,
    pub high: Money,
    pub low: Money,
    pub close: Money,
    pub volume: Count,
    pub right: Option<OptionRight>,
    pub strike: Option<Money>,
    pub open_interest: Option<Money>,
}
pub(crate) fn candle(parts: CandleParts) -> Candle {
    Candle {
        exchange: parts.exchange,
        stock_code: parts.stock_code,
        interval: parts.interval,
        open: parts.open,
        high: parts.high,
        low: parts.low,
        close: parts.close,
        volume: parts.volume,
        right: parts.right,
        strike: parts.strike,
        open_interest: parts.open_interest,
    }
}
pub(crate) fn raw(value: serde_json::Value) -> RawStreamFrame {
    RawStreamFrame(value)
}
