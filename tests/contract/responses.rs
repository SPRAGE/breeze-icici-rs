use breeze_icici::EndpointRequest;
use breeze_icici::account::{GetDematHoldings, GetFunds, GetMarginRequest};
use breeze_icici::domain::{Exchange, UnknownValue};
use breeze_icici::orders::OrderStatus;
use breeze_icici::portfolio::GetPositions;
use breeze_icici::testing::decode_response;
use http::{HeaderMap, StatusCode};
use serde_json::{Value, json};

use crate::rest_wire::{
    cancel_order, customer_details, gtt_cancel, gtt_list, gtt_modify, gtt_place, historical_v1,
    historical_v2, holdings, limit_price, margin_calculation, modify_order, option_chain,
    order_detail, order_list, place_order, preview_equity, preview_fno, quote, set_funds,
    square_off, trade_detail, trade_list,
};
use crate::support::response_fixture;

fn assert_fixture_decodes<R: EndpointRequest>(id: &str, request: R) {
    let bytes = response_fixture(id);
    let _: R::Response = decode_response(&request, StatusCode::OK, &HeaderMap::new(), &bytes)
        .unwrap_or_else(|error| panic!("{id} fixture failed to decode: {error:?}"));
}

#[test]
fn all_twenty_seven_documented_success_shapes_decode_through_their_request_type() {
    assert_fixture_decodes("auth.customer_details", customer_details());
    assert_fixture_decodes("account.demat_holdings", GetDematHoldings);
    assert_fixture_decodes("account.get_funds", GetFunds);
    assert_fixture_decodes("account.set_funds", set_funds());
    assert_fixture_decodes("market.historical_v1", historical_v1());
    assert_fixture_decodes("risk.margin_calculator", margin_calculation());
    assert_fixture_decodes("account.get_margin", GetMarginRequest::new(Exchange::Nse));
    assert_fixture_decodes("orders.place", place_order());
    assert_fixture_decodes("orders.detail", order_detail());
    assert_fixture_decodes("orders.list", order_list());
    assert_fixture_decodes("orders.cancel", cancel_order());
    assert_fixture_decodes("orders.modify", modify_order());
    assert_fixture_decodes("risk.limit_price", limit_price());
    assert_fixture_decodes("portfolio.holdings", holdings());
    assert_fixture_decodes("portfolio.positions", GetPositions);
    assert_fixture_decodes("market.quotes", quote());
    assert_fixture_decodes("orders.square_off", square_off());
    assert_fixture_decodes("trades.list", trade_list());
    assert_fixture_decodes("trades.detail", trade_detail());
    assert_fixture_decodes("market.option_chain", option_chain());
    assert_fixture_decodes("orders.preview_equity", preview_equity());
    assert_fixture_decodes("orders.preview_fno", preview_fno());
    assert_fixture_decodes("market.historical_v2", historical_v2());
    assert_fixture_decodes("gtt.place", gtt_place());
    assert_fixture_decodes("gtt.list", gtt_list());
    assert_fixture_decodes("gtt.cancel", gtt_cancel());
    assert_fixture_decodes("gtt.modify", gtt_modify());
}

#[test]
fn customer_details_retains_session_token_for_client_authentication() {
    let response = decode_response(
        &customer_details(),
        StatusCode::OK,
        &HeaderMap::new(),
        &response_fixture("auth.customer_details"),
    )
    .unwrap();

    assert_eq!(response.user_id().as_str(), "USER-TEST");
    assert!(response.session_token().is_some());
    assert!(!format!("{response:?}").contains("dXNlci10ZXN0"));
}

#[test]
fn documented_number_and_numeric_string_shapes_map_to_decimal_values() {
    let funds = decode_response(
        &GetFunds,
        StatusCode::OK,
        &HeaderMap::new(),
        &response_fixture("account.get_funds"),
    )
    .unwrap();
    assert_eq!(funds.total_bank_balance().to_string(), "800000.0");
    assert_eq!(funds.unallocated_balance().to_string(), "87500");

    let quote = decode_response(
        &quote(),
        StatusCode::OK,
        &HeaderMap::new(),
        &response_fixture("market.quotes"),
    )
    .unwrap();
    assert_eq!(quote[0].last_price().to_string(), "23832.85");
    assert_eq!(quote[0].best_bid_quantity().unwrap().get(), 1500);
}

#[test]
fn null_and_empty_string_are_not_coerced_to_zero_without_field_evidence() {
    let orders = decode_response(
        &order_detail(),
        StatusCode::OK,
        &HeaderMap::new(),
        &response_fixture("orders.detail"),
    )
    .unwrap();

    assert_eq!(orders[0].expiry(), None);
    assert_eq!(orders[0].parent_order_id(), None);
    assert_eq!(orders[0].validity_date_raw(), Some(""));
}

#[test]
fn unknown_fields_are_ignored_without_losing_known_fields() {
    let mut fixture: Value =
        serde_json::from_slice(&response_fixture("account.get_funds")).unwrap();
    fixture["Success"]["new_upstream_field"] = json!({"nested": [1, 2, 3]});

    let funds = decode_response(
        &GetFunds,
        StatusCode::OK,
        &HeaderMap::new(),
        &serde_json::to_vec(&fixture).unwrap(),
    )
    .unwrap();
    assert_eq!(funds.unallocated_balance().to_string(), "87500");
}

#[test]
fn unknown_order_status_is_preserved_and_never_mapped_to_a_terminal_state() {
    let mut fixture: Value = serde_json::from_slice(&response_fixture("orders.detail")).unwrap();
    fixture["Success"][0]["status"] = json!("Awaiting Exchange Reconciliation");

    let orders = decode_response(
        &order_detail(),
        StatusCode::OK,
        &HeaderMap::new(),
        &serde_json::to_vec(&fixture).unwrap(),
    )
    .unwrap();

    assert_eq!(
        orders[0].status(),
        &OrderStatus::Other(UnknownValue::new("Awaiting Exchange Reconciliation"))
    );
    assert!(!orders[0].status().is_terminal());
}

#[test]
fn wrong_success_shape_is_a_decode_error_with_endpoint_context() {
    let body = br#"{"Success":{},"Status":200,"Error":null}"#;
    let error =
        decode_response(&order_detail(), StatusCode::OK, &HeaderMap::new(), body).unwrap_err();
    let rendered = format!("{error:?}");
    assert!(rendered.contains("order"));
    assert!(!rendered.contains("secret-key-test"));
}
