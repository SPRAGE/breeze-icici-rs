use std::collections::BTreeMap;

use breeze_icici::EndpointRequest;
use breeze_icici::account::{GetDematHoldings, GetFunds, GetMarginRequest};
use breeze_icici::domain::{DerivativeExchange, Exchange, Interval, Product, UserRemark};
use breeze_icici::gtt::{FreshGttOrder, GttIndexKind, GttLeg, GttLegSet, GttOrderRequest};
use breeze_icici::market::{HistoricalV1Request, HistoricalV2Request, OptionChainRequest};
use breeze_icici::orders::{
    Action, OrderId, OrderSegmentCode, OrderType, PlaceOrderRequest, PreviewOrderRequest,
    SettlementId, Validity,
};
use breeze_icici::portfolio::GetPositions;
use breeze_icici::risk::{LimitPriceRequest, MarginCalculationRequest, MarginPosition, SourceFlag};
use breeze_icici::trades::TradeListRequest;
use serde_json::Value;
use std::str::FromStr;

use crate::rest_wire::*;
use crate::support::{
    date, equity, fixture, future, money, option, prepared, quantity, range, stock,
};

fn assert_sdk<R: EndpointRequest>(id: &str, request: R) {
    let fixture: Value = fixture("sdk_wire_contracts.json");
    assert_eq!(
        fixture["sdk_revision"],
        "4125106b48932ff99b45d593749dcec21c552558"
    );
    let expected = &fixture["cases"][id];
    assert!(expected.is_object(), "missing SDK case {id}");
    let actual = prepared(request);
    let mut actual_url = actual.url().clone();
    actual_url.set_query(None);
    assert_eq!(
        actual_url.as_str(),
        expected["url"].as_str().unwrap(),
        "{id} URL"
    );
    for name in ["content-type", "x-appkey", "apikey", "x-sessiontoken"] {
        assert_eq!(
            actual.header(name),
            expected["headers"][name].as_str(),
            "{id} {name}"
        );
    }
    assert_eq!(
        actual.method().as_str(),
        expected["method"].as_str().unwrap(),
        "{id} method"
    );
    assert!(
        actual
            .url()
            .path()
            .ends_with(expected["path"].as_str().unwrap()),
        "{id} path"
    );
    if expected["body"] == "" {
        assert!(actual.body().is_empty());
        let query: BTreeMap<_, _> = actual
            .url()
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        assert_eq!(
            serde_json::to_value(query).unwrap(),
            expected["query"],
            "{id} query"
        );
        assert_eq!(actual.header("content-type"), Some("application/json"));
    } else {
        // Object ordering does not alter API semantics. Independent exact-byte
        // tests prove that each body's checksum matches its transmitted bytes.
        let actual: Value = serde_json::from_slice(actual.body()).unwrap();
        let sdk: Value = serde_json::from_str(expected["body"].as_str().unwrap()).unwrap();
        assert_eq!(actual, sdk, "{id} SDK payload");
    }
}

#[test]
fn all_existing_rest_operations_match_captured_sdk_requests() {
    assert_sdk("auth.customer_details", customer_details());
    assert_sdk("account.demat_holdings", GetDematHoldings);
    assert_sdk("account.get_funds", GetFunds);
    assert_sdk("account.set_funds", set_funds());
    assert_sdk("market.historical_v1", historical_v1());
    assert_sdk("risk.margin_calculator", margin_calculation());
    assert_sdk("account.get_margin", GetMarginRequest::new(Exchange::Nse));
    assert_sdk("orders.place", place_order());
    assert_sdk("orders.detail", order_detail());
    assert_sdk("orders.list", order_list());
    assert_sdk("orders.cancel", cancel_order());
    assert_sdk("orders.modify", modify_order());
    assert_sdk("risk.limit_price", limit_price());
    assert_sdk("portfolio.holdings", holdings());
    assert_sdk("portfolio.positions", GetPositions);
    assert_sdk("market.quotes", quote());
    assert_sdk("orders.square_off", square_off());
    assert_sdk("trades.list", trade_list());
    assert_sdk("trades.detail", trade_detail());
    assert_sdk("market.option_chain", option_chain());
    assert_sdk("orders.preview_equity", preview_equity());
    assert_sdk("orders.preview_fno", preview_fno());
    assert_sdk("market.historical_v2", historical_v2());
    assert_sdk("gtt.place", gtt_place());
    assert_sdk("gtt.list", gtt_list());
    assert_sdk("gtt.cancel", gtt_cancel());
    assert_sdk("gtt.modify", gtt_modify());
}

#[test]
fn sdk_optional_filters_intervals_routing_and_selectors_match() {
    let dates = range("2025-02-03T09:15:00.000Z", "2025-02-03T15:30:00.000Z");
    assert_sdk(
        "sdk.history_second",
        HistoricalV2Request::new(Interval::OneSecond, dates.clone(), option()).unwrap(),
    );
    assert!(HistoricalV1Request::new(Interval::OneSecond, dates.clone(), option()).is_err());
    let bfo = breeze_icici::domain::Instrument::future(
        DerivativeExchange::Bfo,
        stock("BSESEN"),
        date("2025-02-27"),
    )
    .unwrap();
    assert!(HistoricalV1Request::new(Interval::OneMinute, dates, bfo).is_err());
    assert_sdk(
        "sdk.trades_all",
        TradeListRequest::all(Exchange::Nse).build().unwrap(),
    );
    assert_sdk(
        "sdk.futures_all",
        OptionChainRequest::futures(DerivativeExchange::Nfo, stock("NIFTY"))
            .build()
            .unwrap(),
    );
    assert_sdk("sdk.gtt_list_bfo", gtt_list().exchange(Exchange::Bfo));
    assert_sdk("sdk.gtt_cancel_bfo", gtt_cancel().exchange(Exchange::Bfo));
    assert_sdk("sdk.gtt_modify_bfo", gtt_modify().exchange(Exchange::Bfo));
    let fixture: Value = fixture("sdk_wire_contracts.json");
    assert_eq!(fixture["cases"].as_object().unwrap().len(), 44);
    assert_eq!(
        fixture["cases"]["orders.modify"]["preflight"],
        serde_json::json!([
            {"method":"GET", "path":"/order", "body":{"exchange_code":"NSE", "order_id":"ORDER-TEST-1"}}
        ])
    );
    assert_eq!(
        fixture["cases"]["sdk.limit_blank_product"]["rejected"],
        true
    );
    assert!(Product::from_str("").is_err());
    assert_sdk(
        "sdk.limit_source_m",
        LimitPriceRequest::builder(option(), Action::Buy)
            .stop_loss_trigger(money("5.00"))
            .source_flag(SourceFlag::M)
            .limit_rate(money("6.00"))
            .order_reference(OrderId::new("ORDER-TEST-1").unwrap())
            .available_quantity(quantity(75))
            .fresh_order_limit(money("6.00"))
            .build()
            .unwrap(),
    );
    assert_sdk(
        "sdk.preview_stoploss_blank",
        PreviewOrderRequest::limit(equity(), Action::Buy, quantity(1), money("420.00"))
            .order_type(OrderType::StopLoss)
            .build()
            .unwrap(),
    );
    let place = || {
        PlaceOrderRequest::limit(equity(), Action::Buy, quantity(1), money("420.00"))
            .user_remark(UserRemark::new("rustsdk").unwrap())
    };
    assert_sdk(
        "sdk.place_vtc",
        place()
            .validity(Validity::ValidTillCancelled)
            .build()
            .unwrap(),
    );
    assert_sdk(
        "sdk.place_routing",
        place()
            .validity(Validity::Day)
            .settlement_id(SettlementId::new("SETTLEMENT-TEST").unwrap())
            .order_segment_code(OrderSegmentCode::new("NSE").unwrap())
            .lots(quantity(2))
            .build()
            .unwrap(),
    );
}

#[test]
fn sdk_independent_cover_product_and_history_variants_match() {
    let margin = MarginPosition::new(option(), Action::Buy, quantity(75), money("245.05"))
        .cover_order_flow(Action::Sell)
        .fresh_order_type(OrderType::Limit)
        .cover_limit_rate(money("180.00"))
        .cover_stop_loss_trigger(money("200.00"))
        .fresh_limit_rate(money("245.05"))
        .open_quantity(quantity(75));
    assert_sdk(
        "sdk.margin_cover",
        MarginCalculationRequest::new(vec![margin]).unwrap(),
    );
    let legs = GttLegSet::cover_oco(
        GttLeg::target(Action::Sell, money("25000"), money("24900")).unwrap(),
        GttLeg::stop_loss(Action::Sell, money("22000"), money("22100")).unwrap(),
    )
    .unwrap();
    assert_sdk(
        "sdk.gtt_futures",
        GttOrderRequest::cover_oco(
            future(),
            quantity(75),
            FreshGttOrder::limit(Action::Buy, money("23400")),
            legs,
        )
        .index_kind(GttIndexKind::Index)
        .trade_date(date("2025-02-05"))
        .build()
        .unwrap(),
    );
    assert_sdk(
        "sdk.btst_placement",
        PlaceOrderRequest::limit(equity(), Action::Buy, quantity(1), money("420.00"))
            .product(Product::Btst)
            .validity(Validity::Day)
            .user_remark(UserRemark::new("rustsdk").unwrap())
            .build()
            .unwrap(),
    );
    assert_sdk(
        "sdk.mtf_preview",
        PreviewOrderRequest::limit(equity(), Action::Buy, quantity(1), money("420.00"))
            .product(Product::Mtf)
            .build()
            .unwrap(),
    );
    for (id, interval) in [
        ("sdk.history_five", Interval::FiveMinutes),
        ("sdk.history_thirty", Interval::ThirtyMinutes),
    ] {
        assert_sdk(
            id,
            HistoricalV2Request::new(
                interval,
                range("2025-02-03T09:15:00.000Z", "2025-02-03T15:30:00.000Z"),
                option(),
            )
            .unwrap(),
        );
    }
}
