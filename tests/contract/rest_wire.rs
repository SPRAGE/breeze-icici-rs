use std::collections::BTreeMap;
use std::fmt::Write as _;

use breeze_icici::EndpointRequest;
use breeze_icici::account::{
    CustomerDetailsRequest, FundAmount, FundSegment, FundTransaction, GetDematHoldings, GetFunds,
    GetMarginRequest, SetFundsRequest,
};
use breeze_icici::auth::{ApiSession, AppKey};
use breeze_icici::domain::{Exchange, Interval, UserRemark};
use breeze_icici::gtt::{
    CancelGttOrderRequest, FreshGttOrder, GttIndexKind, GttLeg, GttLegSet, GttOrderId,
    GttOrderListRequest, GttOrderRequest, ModifyGttOrderRequest,
};
use breeze_icici::market::{
    HistoricalV1Request, HistoricalV2Request, OptionChainRequest, QuoteRequest,
};
use breeze_icici::orders::{
    Action, CancelOrderRequest, ModifyOrderRequest, OrderDetailRequest, OrderId, OrderListRequest,
    OrderType, PlaceOrderRequest, PreviewOrderRequest, SquareOffRequest, Validity,
};
use breeze_icici::portfolio::{GetPositions, HoldingsRequest, PortfolioType};
use breeze_icici::risk::{LimitPriceRequest, MarginCalculationRequest, MarginPosition, SourceFlag};
use breeze_icici::trades::{TradeDetailRequest, TradeListRequest};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::support::{
    API_SESSION, APP_KEY, FIXED_TIMESTAMP, SECRET_KEY, SESSION_TOKEN, date, equity, fixture,
    future, money, option, prepared, quantity, range, stock,
};

#[derive(Debug, Deserialize)]
struct ExpectedWire {
    method: String,
    base: String,
    path: String,
    auth: String,
    body: String,
    #[serde(default)]
    query: Vec<(String, String)>,
}

fn expected_wire(id: &str) -> ExpectedWire {
    let contracts: BTreeMap<String, ExpectedWire> = fixture("wire_contracts.json");
    contracts.into_iter().find(|(key, _)| key == id).unwrap().1
}

fn expected_checksum(body: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(FIXED_TIMESTAMP.as_bytes());
    hasher.update(body);
    hasher.update(SECRET_KEY.as_bytes());
    let mut digest = String::with_capacity(64);
    for byte in hasher.finalize() {
        write!(&mut digest, "{byte:02x}").unwrap();
    }
    format!("token {digest}")
}

fn assert_wire<R: EndpointRequest>(id: &str, request: R) {
    let expected = expected_wire(id);
    let actual = prepared(request);

    assert_eq!(actual.method().as_str(), expected.method, "{id} method");
    assert!(
        actual.url().path().ends_with(&expected.path),
        "{id} path: {} does not end with {}",
        actual.url().path(),
        expected.path
    );
    assert_eq!(
        actual.body(),
        expected.body.as_bytes(),
        "{id} exact body bytes"
    );

    let actual_query: Vec<_> = actual
        .url()
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    assert_eq!(actual_query, expected.query, "{id} ordered query");

    match expected.auth.as_str() {
        "session_exchange" => {
            assert_eq!(actual.header("x-checksum"), None);
            assert_eq!(actual.header("x-timestamp"), None);
            assert_eq!(actual.header("x-sessiontoken"), None);
        }
        "signed_v1" => {
            assert_eq!(expected.base, "rest_v1");
            assert_eq!(actual.header("content-type"), Some("application/json"));
            assert_eq!(actual.header("x-timestamp"), Some(FIXED_TIMESTAMP));
            assert_eq!(actual.header("x-appkey"), Some(APP_KEY));
            assert_eq!(actual.header("x-sessiontoken"), Some(SESSION_TOKEN));
            assert_eq!(
                actual.header("x-checksum"),
                Some(expected_checksum(expected.body.as_bytes()).as_str())
            );
        }
        "session_v2" => {
            assert_eq!(expected.base, "rest_v2");
            assert!(actual.body().is_empty());
            assert_eq!(actual.header("x-sessiontoken"), Some(SESSION_TOKEN));
            assert_eq!(actual.header("apikey"), Some(APP_KEY));
            assert_eq!(actual.header("x-checksum"), None);
            assert_eq!(actual.header("x-timestamp"), None);
        }
        other => panic!("unexpected auth mode {other}"),
    }
}

pub(crate) fn customer_details() -> CustomerDetailsRequest {
    CustomerDetailsRequest::new(
        AppKey::new(APP_KEY).unwrap(),
        ApiSession::new(API_SESSION).unwrap(),
    )
}

pub(crate) fn set_funds() -> SetFundsRequest {
    SetFundsRequest::new(
        FundTransaction::Credit,
        FundAmount::new(10_000).unwrap(),
        FundSegment::FuturesAndOptions,
    )
}

pub(crate) fn historical_v1() -> HistoricalV1Request {
    HistoricalV1Request::new(
        Interval::OneMinute,
        range("2025-02-03T09:15:00.000Z", "2025-02-03T15:30:00.000Z"),
        equity(),
    )
    .unwrap()
}

pub(crate) fn margin_calculation() -> MarginCalculationRequest {
    let position = MarginPosition::new(future(), Action::Buy, quantity(75), money("23400"));
    MarginCalculationRequest::new(vec![position]).unwrap()
}

pub(crate) fn place_order() -> PlaceOrderRequest {
    PlaceOrderRequest::limit(equity(), Action::Buy, quantity(1), money("420.00"))
        .validity(Validity::Day)
        .user_remark(UserRemark::new("rustsdk").unwrap())
        .build()
        .unwrap()
}

pub(crate) fn order_detail() -> OrderDetailRequest {
    OrderDetailRequest::new(Exchange::Nse, OrderId::new("ORDER-TEST-1").unwrap())
}

pub(crate) fn order_list() -> OrderListRequest {
    OrderListRequest::new(
        Exchange::Nse,
        range("2025-02-01T00:00:00.000Z", "2025-02-05T23:59:59.000Z"),
    )
    .unwrap()
}

pub(crate) fn cancel_order() -> CancelOrderRequest {
    CancelOrderRequest::new(OrderId::new("ORDER-TEST-1").unwrap(), Exchange::Nse)
}

pub(crate) fn modify_order() -> ModifyOrderRequest {
    ModifyOrderRequest::builder(Exchange::Nse, OrderId::new("ORDER-TEST-1").unwrap())
        .quantity(quantity(2))
        .price(money("421.00"))
        .build()
        .unwrap()
}

pub(crate) fn limit_price() -> LimitPriceRequest {
    LimitPriceRequest::builder(option(), Action::Buy)
        .stop_loss_trigger(money("5.00"))
        .source_flag(SourceFlag::Portfolio)
        .limit_rate(money("6.00"))
        .order_reference(OrderId::new("ORDER-TEST-1").unwrap())
        .available_quantity(quantity(75))
        .fresh_order_limit(money("6.00"))
        .build()
        .unwrap()
}

pub(crate) fn holdings() -> HoldingsRequest {
    HoldingsRequest::builder(Exchange::Nse)
        .date_range(range(
            "2025-02-01T00:00:00.000Z",
            "2025-02-05T23:59:59.000Z",
        ))
        .stock_code(stock("ITC"))
        .build()
        .unwrap()
}

pub(crate) fn quote() -> QuoteRequest {
    QuoteRequest::new(equity())
}

pub(crate) fn square_off() -> SquareOffRequest {
    SquareOffRequest::limit(option(), Action::Sell, quantity(75), money("6.00"))
        .validity(Validity::Day)
        .open_quantity(quantity(75))
        .build()
        .unwrap()
}

pub(crate) fn trade_list() -> TradeListRequest {
    TradeListRequest::builder(
        Exchange::Nse,
        range("2025-02-01T00:00:00.000Z", "2025-02-05T23:59:59.000Z"),
    )
    .product(breeze_icici::domain::Product::Cash)
    .action(Action::Buy)
    .stock_code(stock("ITC"))
    .build()
    .unwrap()
}

pub(crate) fn trade_detail() -> TradeDetailRequest {
    TradeDetailRequest::new(Exchange::Nse, OrderId::new("ORDER-TEST-1").unwrap())
}

pub(crate) fn option_chain() -> OptionChainRequest {
    OptionChainRequest::try_from(option()).unwrap()
}

pub(crate) fn preview_equity() -> PreviewOrderRequest {
    PreviewOrderRequest::limit(equity(), Action::Buy, quantity(1), money("420.00"))
        .build()
        .unwrap()
}

pub(crate) fn preview_fno() -> PreviewOrderRequest {
    PreviewOrderRequest::limit(option(), Action::Buy, quantity(75), money("6.00"))
        .stop_loss(money("5.00"))
        .fresh_order_rate(money("6.00"))
        .build()
        .unwrap()
}

pub(crate) fn historical_v2() -> HistoricalV2Request {
    HistoricalV2Request::new(
        Interval::OneMinute,
        range("2025-02-03T09:15:00.000Z", "2025-02-03T15:30:00.000Z"),
        option(),
    )
    .unwrap()
}

fn gtt_legs(
    target_price: &str,
    target_trigger: &str,
    stop_price: &str,
    stop_trigger: &str,
) -> GttLegSet {
    GttLegSet::cover_oco(
        GttLeg::target(Action::Sell, money(target_price), money(target_trigger)).unwrap(),
        GttLeg::stop_loss(Action::Sell, money(stop_price), money(stop_trigger)).unwrap(),
    )
    .unwrap()
}

pub(crate) fn gtt_place() -> GttOrderRequest {
    GttOrderRequest::cover_oco(
        option(),
        quantity(75),
        FreshGttOrder::limit(Action::Buy, money("6.00")),
        gtt_legs("12.00", "11.50", "4.00", "5.00"),
    )
    .index_kind(GttIndexKind::Index)
    .trade_date(date("2025-02-05"))
    .build()
    .unwrap()
}

pub(crate) fn gtt_list() -> GttOrderListRequest {
    GttOrderListRequest::new(range(
        "2025-02-01T00:00:00.000Z",
        "2025-02-05T23:59:59.000Z",
    ))
    .unwrap()
}

pub(crate) fn gtt_cancel() -> CancelGttOrderRequest {
    CancelGttOrderRequest::new(GttOrderId::new("GTT-TEST-1").unwrap())
}

pub(crate) fn gtt_modify() -> ModifyGttOrderRequest {
    ModifyGttOrderRequest::cover_oco(
        GttOrderId::new("GTT-TEST-1").unwrap(),
        gtt_legs("13.00", "12.50", "3.50", "4.50"),
    )
}

#[test]
fn authentication_and_account_wire_contracts() {
    assert_wire("auth.customer_details", customer_details());
    assert_wire("account.demat_holdings", GetDematHoldings);
    assert_wire("account.get_funds", GetFunds);
    assert_wire("account.set_funds", set_funds());
    assert_wire("account.get_margin", GetMarginRequest::new(Exchange::Nse));
}

#[test]
fn market_data_and_risk_wire_contracts() {
    assert_wire("market.historical_v1", historical_v1());
    assert_wire("market.historical_v2", historical_v2());
    assert_wire("market.quotes", quote());
    assert_wire("market.option_chain", option_chain());
    assert_wire("risk.margin_calculator", margin_calculation());
    assert_wire("risk.limit_price", limit_price());
}

#[test]
fn derivative_reads_preserve_the_complete_contract_identity() {
    let historical = HistoricalV1Request::new(
        Interval::OneMinute,
        range("2025-02-03T09:15:00.000Z", "2025-02-03T15:30:00.000Z"),
        option(),
    )
    .unwrap();
    assert_eq!(
        prepared(historical).body(),
        br#"{"interval":"minute","from_date":"2025-02-03T09:15:00.000Z","to_date":"2025-02-03T15:30:00.000Z","stock_code":"NIFTY","exchange_code":"NFO","product_type":"options","expiry_date":"2025-02-27T00:00:00.000Z","strike_price":"24000","right":"call"}"#
    );

    assert_eq!(
        prepared(QuoteRequest::new(option())).body(),
        br#"{"stock_code":"NIFTY","exchange_code":"NFO","expiry_date":"2025-02-27T00:00:00.000Z","product_type":"options","right":"call","strike_price":"24000"}"#
    );

    assert_eq!(
        prepared(QuoteRequest::new(future())).body(),
        br#"{"stock_code":"NIFTY","exchange_code":"NFO","expiry_date":"2025-02-27T00:00:00.000Z","product_type":"futures"}"#
    );
}

#[test]
fn order_wire_contracts() {
    assert_wire("orders.place", place_order());
    assert_wire("orders.detail", order_detail());
    assert_wire("orders.list", order_list());
    assert_wire("orders.cancel", cancel_order());
    assert_wire("orders.modify", modify_order());
    assert_wire("orders.square_off", square_off());
    assert_wire("orders.preview_equity", preview_equity());
    assert_wire("orders.preview_fno", preview_fno());
}

#[test]
fn documented_stop_loss_mutations_have_explicit_exact_wire_shapes() {
    let place = PlaceOrderRequest::stop_loss(
        equity(),
        Action::Sell,
        quantity(1),
        money("419.00"),
        money("420.00"),
    )
    .validity(Validity::Day)
    .build()
    .unwrap();
    let place_prepared = prepared(place);
    let body = br#"{"stock_code":"ITC","exchange_code":"NSE","product":"cash","action":"sell","order_type":"stoploss","quantity":"1","price":"419.00","validity":"day","stoploss":"420.00"}"#;
    assert_eq!(place_prepared.body(), body);
    assert_eq!(
        place_prepared.header("x-checksum"),
        Some(expected_checksum(body).as_str())
    );

    let modify = ModifyOrderRequest::builder(Exchange::Nse, OrderId::new("ORDER-TEST-1").unwrap())
        .order_type(OrderType::StopLoss)
        .stop_loss(money("420.00"))
        .validity(Validity::Day)
        .disclosed_quantity(quantity(1))
        .build()
        .unwrap();
    assert_eq!(
        prepared(modify).body(),
        br#"{"order_id":"ORDER-TEST-1","exchange_code":"NSE","order_type":"stoploss","stoploss":"420.00","validity":"day","disclosed_quantity":"1"}"#
    );

    let square_off = SquareOffRequest::stop_loss(
        option(),
        Action::Sell,
        quantity(75),
        money("5.00"),
        money("6.00"),
    )
    .validity(Validity::Day)
    .open_quantity(quantity(75))
    .build()
    .unwrap();
    let body = String::from_utf8(prepared(square_off).body().to_vec()).unwrap();
    assert!(body.contains(r#""order_type":"stoploss""#));
    assert!(body.contains(r#""stoploss_price":"6.00""#));
}

#[test]
fn documented_disclosed_quantity_is_explicit_on_order_mutations() {
    let place = PlaceOrderRequest::limit(equity(), Action::Buy, quantity(5), money("420.00"))
        .validity(Validity::Day)
        .disclosed_quantity(quantity(2))
        .build()
        .unwrap();
    let place = String::from_utf8(prepared(place).body().to_vec()).unwrap();
    assert!(place.contains(r#""disclosed_quantity":"2""#));

    let square_off = SquareOffRequest::limit(equity(), Action::Sell, quantity(5), money("420.00"))
        .validity(Validity::Day)
        .open_quantity(quantity(5))
        .disclosed_quantity(quantity(2))
        .build()
        .unwrap();
    let square_off = String::from_utf8(prepared(square_off).body().to_vec()).unwrap();
    assert!(square_off.contains(r#""disclosed_quantity":"2""#));
}

#[test]
fn portfolio_and_trade_wire_contracts() {
    assert_wire("portfolio.holdings", holdings());
    assert_wire("portfolio.positions", GetPositions);
    assert_wire("trades.list", trade_list());
    assert_wire("trades.detail", trade_detail());
}

#[test]
fn documented_portfolio_type_filter_is_preserved_when_supplied() {
    let request = HoldingsRequest::builder(Exchange::Nse)
        .portfolio_type(PortfolioType::new("long_term").unwrap())
        .build()
        .unwrap();

    assert_eq!(
        prepared(request).body(),
        br#"{"exchange_code":"NSE","portfolio_type":"long_term"}"#
    );
}

#[test]
fn gtt_wire_contracts() {
    assert_wire("gtt.place", gtt_place());
    assert_wire("gtt.list", gtt_list());
    assert_wire("gtt.cancel", gtt_cancel());
    assert_wire("gtt.modify", gtt_modify());
}

#[test]
fn documented_single_leg_gtt_has_an_explicit_exact_wire_shape() {
    let leg = GttLeg::target(Action::Sell, money("12.00"), money("11.50")).unwrap();
    let place = GttOrderRequest::single(option(), quantity(75), leg.clone())
        .index_kind(GttIndexKind::Index)
        .trade_date(date("2025-02-05"))
        .build()
        .unwrap();
    let place = prepared(place);
    assert_eq!(place.method().as_str(), "POST");
    assert_eq!(
        place.body(),
        br#"{"exchange_code":"NFO","stock_code":"NIFTY","product":"options","quantity":"75","expiry_date":"2025-02-27T00:00:00.000Z","right":"call","strike_price":"24000","gtt_type":"single","index_or_stock":"index","trade_date":"2025-02-05T00:00:00.000Z","order_details":[{"gtt_leg_type":"target","action":"sell","limit_price":"12.00","trigger_price":"11.50"}]}"#
    );

    let modify = prepared(ModifyGttOrderRequest::single(
        GttOrderId::new("GTT-TEST-1").unwrap(),
        leg,
    ));
    assert_eq!(modify.method().as_str(), "PUT");
    assert_eq!(
        modify.body(),
        br#"{"exchange_code":"NFO","gtt_order_id":"GTT-TEST-1","gtt_type":"single","order_details":[{"gtt_leg_type":"target","action":"sell","limit_price":"12.00","trigger_price":"11.50"}]}"#
    );
}
