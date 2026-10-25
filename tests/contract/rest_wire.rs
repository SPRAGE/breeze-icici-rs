use std::collections::BTreeMap;
use std::fmt::Write as _;

use breeze_icici::EndpointRequest;
use breeze_icici::account::{
    CustomerDetailsRequest, FundAmount, FundSegment, FundTransaction, GetDematHoldings, GetFunds,
    GetMarginRequest, SetFundsRequest,
};
use breeze_icici::auth::{ApiSession, AppKey};
use breeze_icici::domain::{Count, Exchange, Interval, Product, UserRemark};
use breeze_icici::gtt::{
    CancelGttOrderRequest, FreshGttOrder, GttIndexKind, GttLeg, GttLegSet, GttOrderId,
    GttOrderListRequest, GttOrderRequest, ModifyGttOrderRequest,
};
use breeze_icici::market::{
    HistoricalV1Request, HistoricalV2Request, OptionChainRequest, QuoteRequest,
};
use breeze_icici::orders::{
    Action, CancelOrderRequest, ModifyOrderRequest, OrderDetailRequest, OrderId, OrderListRequest,
    OrderType, PlaceOrderRequest, PreviewFlag, PreviewOrderRequest, SquareOffRequest, Validity,
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
    let body = br#"{"stock_code":"ITC","exchange_code":"NSE","product":"cash","action":"sell","order_type":"stoploss","quantity":"1","price":"419.00","validity":"day","settlement_id":"","order_segment_code":"","lots":"","user_remark":"","stoploss":"420.00"}"#;
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

#[test]
fn api_review_fresh_limit_calculation_needs_no_existing_order() {
    let request = LimitPriceRequest::builder(option(), Action::Buy)
        .stop_loss_trigger(money("0"))
        .source_flag(SourceFlag::Portfolio)
        .available_quantity(quantity(75))
        .fresh_order_limit(money("85"))
        .build()
        .unwrap();
    let actual = prepared(request);
    let body = br#"{"strike_price":"24000","product_type":"options","expiry_date":"2025-02-27T00:00:00.000Z","underlying":"NIFTY","exchange_code":"NFO","order_flow":"buy","stop_loss_trigger":"0","option_type":"call","source_flag":"P","limit_rate":"","order_reference":"","available_quantity":"75","market_type":"limit","fresh_order_limit":"85"}"#;
    assert_eq!(actual.body(), body);
    assert_eq!(
        actual.header("x-checksum"),
        Some(expected_checksum(body).as_str())
    );
}

#[test]
fn api_review_futures_option_chain_is_representable() {
    let request = OptionChainRequest::try_from(future()).unwrap();
    assert_eq!(prepared(request).body(), br#"{"stock_code":"NIFTY","exchange_code":"NFO","expiry_date":"2025-02-27T00:00:00.000Z","product_type":"futures"}"#);
    let request = OptionChainRequest::futures(
        breeze_icici::domain::DerivativeExchange::Bfo,
        stock("BSESEN"),
    )
    .expiry(date("2025-02-27"))
    .build()
    .unwrap();
    assert_eq!(prepared(request).body(), br#"{"stock_code":"BSESEN","exchange_code":"BFO","expiry_date":"2025-02-27T00:00:00.000Z","product_type":"futures"}"#);
}

#[test]
fn api_review_margin_positions_send_documented_neutral_cover_fields() {
    let position = MarginPosition::new(future(), Action::Buy, quantity(75), money("23400"))
        .include_cover_fields();
    let request = MarginCalculationRequest::new(vec![position]).unwrap();
    let body: serde_json::Value = serde_json::from_slice(prepared(request).body()).unwrap();
    let position = &body["list_of_positions"][0];
    for (field, value) in [
        ("cover_order_flow", "N"),
        ("fresh_order_type", "N"),
        ("cover_limit_rate", "0"),
        ("cover_sltp_price", "0"),
        ("fresh_limit_rate", "0"),
        ("open_quantity", "0"),
    ] {
        assert_eq!(position[field], value, "{field}");
    }
}

#[test]
fn api_review_preview_uses_the_documented_normal_flag() {
    let request = PreviewOrderRequest::limit(equity(), Action::Buy, quantity(1), money("420"))
        .special_flag(PreviewFlag::Normal)
        .build()
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(prepared(request).body()).unwrap();
    assert_eq!(body["specialflag"], "N");
}

#[test]
fn api_review_futures_cover_oco_is_representable() {
    let request = GttOrderRequest::cover_oco(
        future(),
        quantity(75),
        FreshGttOrder::limit(Action::Buy, money("23400")),
        gtt_legs("25000", "24900", "22000", "22100"),
    )
    .index_kind(GttIndexKind::Index)
    .trade_date(date("2025-02-05"))
    .build()
    .unwrap();
    let body: serde_json::Value = serde_json::from_slice(prepared(request).body()).unwrap();
    assert_eq!(body["product"], "futures");
    assert_eq!(body["right"], "others");
    assert_eq!(body["strike_price"], "0");
    assert_eq!(body["order_details"].as_array().unwrap().len(), 2);
}

#[test]
fn api_review_historical_five_and_thirty_minute_wires_use_the_correct_protocol() {
    for (interval, wire) in [
        (Interval::FiveMinutes, "5minute"),
        (Interval::ThirtyMinutes, "30minute"),
    ] {
        let dates = range("2025-02-03T09:15:00.000Z", "2025-02-03T15:30:00.000Z");
        let v1 = prepared(HistoricalV1Request::new(interval, dates.clone(), equity()).unwrap());
        let body: serde_json::Value = serde_json::from_slice(v1.body()).unwrap();
        assert_eq!(body["interval"], wire);
        assert_eq!(
            v1.header("x-checksum"),
            Some(expected_checksum(v1.body()).as_str())
        );
        let v2 = prepared(HistoricalV2Request::new(interval, dates, equity()).unwrap());
        assert!(
            v2.url()
                .query_pairs()
                .any(|(key, value)| key == "interval" && value == wire)
        );
        assert!(v2.body().is_empty());
        assert_eq!(v2.header("x-checksum"), None);
    }
}

#[test]
fn api_review_calculator_quantities_distinguish_blank_zero_and_positive() {
    for available in [None, Some(Count::new(0)), Some(Count::new(75))] {
        let mut builder = LimitPriceRequest::builder(option(), Action::Buy)
            .stop_loss_trigger(money("0"))
            .source_flag(SourceFlag::Portfolio)
            .fresh_order_limit(money("85"));
        if let Some(value) = available {
            builder = builder.available_quantity(value);
        }
        let request = builder.build().unwrap();
        let body: serde_json::Value = serde_json::from_slice(prepared(request).body()).unwrap();
        assert_eq!(
            body["available_quantity"],
            available.map_or_else(String::new, |n| n.get().to_string())
        );
        assert_eq!(body["order_reference"], "");
        assert_eq!(body["limit_rate"], "");
    }
    let request = LimitPriceRequest::builder(option(), Action::Buy)
        .product_type(Product::Options)
        .stop_loss_trigger(money("0"))
        .source_flag(SourceFlag::Portfolio)
        .fresh_order_limit(money("85"))
        .build()
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(prepared(request).body()).unwrap();
    assert_eq!(body["product_type"], "options");
}

#[test]
fn api_review_margin_cover_fields_preserve_nonzero_values() {
    let position = MarginPosition::new(option(), Action::Buy, quantity(75), money("245.05"))
        .cover_order_flow(Action::Sell)
        .fresh_order_type(OrderType::Limit)
        .cover_limit_rate(money("180.00"))
        .cover_stop_loss_trigger(money("200.00"))
        .fresh_limit_rate(money("245.05"))
        .open_quantity(quantity(75));
    let actual = prepared(MarginCalculationRequest::new(vec![position]).unwrap());
    let body = br#"{"list_of_positions":[{"strike_price":"24000","quantity":"75","right":"call","product":"options","action":"buy","price":"245.05","expiry_date":"2025-02-27T00:00:00.000Z","stock_code":"NIFTY","cover_order_flow":"sell","fresh_order_type":"limit","cover_limit_rate":"180.00","cover_sltp_price":"200.00","fresh_limit_rate":"245.05","open_quantity":"75"}],"exchange_code":"NFO"}"#;
    assert_eq!(actual.body(), body);
    assert_eq!(
        actual.header("x-checksum"),
        Some(expected_checksum(body).as_str())
    );
}

#[test]
fn api_review_btst_placement_and_btst_mtf_stoploss_previews_are_explicit() {
    let place = PlaceOrderRequest::limit(equity(), Action::Buy, quantity(1), money("420"))
        .product(Product::Btst)
        .validity(Validity::Day)
        .build()
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(prepared(place).body()).unwrap();
    assert_eq!(body["product"], "btst");
    assert!(body.get("expiry_date").is_none());

    for (product, wire) in [(Product::Btst, "btst"), (Product::Mtf, "mtf")] {
        let preview = PreviewOrderRequest::limit(equity(), Action::Buy, quantity(1), money("420"))
            .product(product)
            .order_type(OrderType::StopLoss)
            .stop_loss(money("419"))
            .special_flag(PreviewFlag::Special)
            .fresh_order_rate(money("420"))
            .build()
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(prepared(preview).body()).unwrap();
        assert_eq!(body["product"], wire);
        assert_eq!(body["order_type"], "stoploss");
        assert_eq!(body["specialflag"], "S");
        assert_eq!(body["stoploss"], "419");
    }
}

#[test]
fn api_review_documented_preview_prices_do_not_inherit_mutation_ordering() {
    for order_type in [OrderType::Limit, OrderType::StopLoss] {
        let preview = PreviewOrderRequest::limit(option(), Action::Buy, quantity(75), money("171"))
            .order_type(order_type)
            .special_flag(PreviewFlag::Special)
            .stop_loss(money("190"))
            .fresh_order_rate(money("195.50"))
            .build()
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(prepared(preview).body()).unwrap();
        assert_eq!(body["price"], "171");
        assert_eq!(body["stoploss"], "190");
        assert_eq!(body["order_rate_fresh"], "195.50");
        assert_eq!(body["specialflag"], "S");
    }
}
