use std::str::FromStr;

use breeze_icici::account::{FundAmount, FundSegment, FundTransaction, SetFundsRequest};
use breeze_icici::domain::{
    DateRange, DerivativeExchange, Exchange, Money, OptionRight, Product, Quantity, StockCode,
    UserRemark,
};
use breeze_icici::gtt::{GttIndexKind, GttLeg, GttLegSet, GttOrderRequest};
use breeze_icici::market::OptionChainRequest;
use breeze_icici::orders::{
    Action, ModifyOrderRequest, OrderId, OrderListRequest, OrderType, PlaceOrderRequest,
    SquareOffRequest, Validity,
};
use breeze_icici::portfolio::PortfolioType;

use crate::support::{date, equity, money, option, quantity, range, stock};

#[test]
fn quantities_are_positive_integers() {
    assert!(Quantity::new(1).is_ok());
    assert!(Quantity::new(u64::MAX).is_ok());
    assert!(Quantity::new(0).is_err());
}

#[test]
fn opaque_portfolio_type_is_nonempty_and_control_free() {
    assert!(PortfolioType::new("long_term").is_ok());
    assert!(PortfolioType::new("").is_err());
    assert!(PortfolioType::new("bad\nvalue").is_err());
}

#[test]
fn request_money_rejects_non_finite_exponent_and_non_numeric_text() {
    for value in ["", "NaN", "inf", "-inf", "1e3", "1,000", "₹10", "--"] {
        assert!(
            Money::from_str(value).is_err(),
            "{value:?} must be rejected"
        );
    }

    assert_eq!(
        Money::from_str("420.00").unwrap().to_wire_string(),
        "420.00"
    );
    assert_eq!(Money::from_str("0.05").unwrap().to_wire_string(), "0.05");
}

#[test]
fn stock_codes_are_trimmed_uppercased_and_ascii_bounded() {
    assert_eq!(StockCode::new(" itc ").unwrap().as_str(), "ITC");
    for value in ["", "   ", "निफ्टी", "ITC/../../secret", "A CODE"] {
        assert!(StockCode::new(value).is_err(), "{value:?} must be rejected");
    }
}

#[test]
fn remarks_follow_the_documented_no_space_or_special_character_rule() {
    assert_eq!(UserRemark::new("rustsdk42").unwrap().as_str(), "rustsdk42");
    for value in ["has space", "has_underscore", "has-dash", "tag!", "टैग"] {
        assert!(
            UserRemark::new(value).is_err(),
            "{value:?} must be rejected"
        );
    }
}

#[test]
fn date_ranges_are_ordered_and_endpoint_windows_are_enforced() {
    assert!(
        DateRange::new(
            crate::support::time("2025-02-05T00:00:00.000Z"),
            crate::support::time("2025-02-01T00:00:00.000Z")
        )
        .is_err()
    );

    let eleven_days = range("2025-02-01T00:00:00.000Z", "2025-02-12T00:00:00.000Z");
    assert!(OrderListRequest::new(Exchange::Nse, eleven_days).is_err());

    let ten_days = range("2025-02-01T00:00:00.000Z", "2025-02-11T00:00:00.000Z");
    assert!(OrderListRequest::new(Exchange::Nse, ten_days).is_ok());
}

#[test]
fn option_chain_requires_two_of_expiry_right_and_strike() {
    let base = || OptionChainRequest::builder(DerivativeExchange::Nfo, stock("NIFTY"));

    assert!(base().expiry(date("2025-02-27")).build().is_err());
    assert!(base().right(OptionRight::Call).build().is_err());
    assert!(base().strike(money("24000")).build().is_err());

    assert!(
        base()
            .expiry(date("2025-02-27"))
            .right(OptionRight::Call)
            .build()
            .is_ok()
    );
    assert!(
        base()
            .expiry(date("2025-02-27"))
            .strike(money("24000"))
            .build()
            .is_ok()
    );
    assert!(
        base()
            .right(OptionRight::Call)
            .strike(money("24000"))
            .build()
            .is_ok()
    );
}

#[test]
fn market_and_prohibited_margin_products_are_not_request_values() {
    assert!(OrderType::from_str("market").is_err());
    assert!(Product::from_str("margin").is_err());
    assert!(Product::from_str("optionplus").is_err());
    assert_eq!(OrderType::from_str("limit").unwrap(), OrderType::Limit);
    assert_eq!(
        OrderType::from_str("stoploss").unwrap(),
        OrderType::StopLoss
    );
}

#[test]
fn place_order_builder_needs_a_complete_explicit_limit_order() {
    let request = PlaceOrderRequest::limit(equity(), Action::Buy, quantity(1), money("420.00"))
        .validity(Validity::Day)
        .user_remark(UserRemark::new("rustsdk").unwrap())
        .build()
        .unwrap();

    assert_eq!(request.instrument(), &equity());
    assert_eq!(request.action(), Action::Buy);
    assert_eq!(request.order_type(), OrderType::Limit);
}

#[test]
fn disclosed_quantity_cannot_exceed_the_order_quantity() {
    assert!(
        PlaceOrderRequest::limit(equity(), Action::Buy, quantity(5), money("420.00"))
            .validity(Validity::Day)
            .disclosed_quantity(quantity(6))
            .build()
            .is_err()
    );
    assert!(
        SquareOffRequest::limit(equity(), Action::Sell, quantity(5), money("420.00"))
            .validity(Validity::Day)
            .open_quantity(quantity(5))
            .disclosed_quantity(quantity(6))
            .build()
            .is_err()
    );
}

#[test]
fn stop_loss_orders_are_explicit_and_validate_limit_trigger_direction() {
    let sell = PlaceOrderRequest::stop_loss(
        equity(),
        Action::Sell,
        quantity(1),
        money("419.00"),
        money("420.00"),
    )
    .validity(Validity::Day)
    .build()
    .unwrap();
    assert_eq!(sell.order_type(), OrderType::StopLoss);
    assert_eq!(sell.stop_loss_trigger(), Some(&money("420.00")));

    assert!(
        PlaceOrderRequest::stop_loss(
            equity(),
            Action::Sell,
            quantity(1),
            money("421.00"),
            money("420.00"),
        )
        .validity(Validity::Day)
        .build()
        .is_err()
    );
    assert!(
        SquareOffRequest::stop_loss(
            option(),
            Action::Buy,
            quantity(75),
            money("5.00"),
            money("6.00"),
        )
        .validity(Validity::Day)
        .open_quantity(quantity(75))
        .build()
        .is_err()
    );
}

#[test]
fn modification_requires_at_least_one_real_change() {
    let builder =
        || ModifyOrderRequest::builder(Exchange::Nse, OrderId::new("ORDER-TEST-1").unwrap());
    assert!(builder().build().is_err());
    assert!(builder().quantity(quantity(2)).build().is_ok());
    assert!(builder().price(money("421.00")).build().is_ok());
    assert!(
        builder()
            .order_type(OrderType::StopLoss)
            .stop_loss(money("420.00"))
            .validity(Validity::Day)
            .disclosed_quantity(quantity(1))
            .build()
            .is_ok()
    );
}

#[test]
fn gtt_cover_oco_requires_one_target_and_one_stoploss_leg() {
    let target = GttLeg::target(Action::Sell, money("12.00"), money("11.50")).unwrap();
    let stop = GttLeg::stop_loss(Action::Sell, money("4.00"), money("5.00")).unwrap();

    assert!(GttLegSet::cover_oco(target.clone(), stop.clone()).is_ok());
    assert!(GttLegSet::cover_oco(target.clone(), target).is_err());
    assert!(GttLegSet::cover_oco(stop.clone(), stop).is_err());
}

#[test]
fn documented_single_leg_gtt_requires_one_derivative_leg() {
    let target = GttLeg::target(Action::Sell, money("12.00"), money("11.50")).unwrap();

    assert!(
        GttOrderRequest::single(option(), quantity(75), target.clone())
            .index_kind(GttIndexKind::Index)
            .trade_date(date("2025-02-05"))
            .build()
            .is_ok()
    );
    assert!(
        GttOrderRequest::single(equity(), quantity(1), target)
            .index_kind(GttIndexKind::Stock)
            .trade_date(date("2025-02-05"))
            .build()
            .is_err()
    );
}

#[test]
fn set_funds_is_positive_whole_rupees_and_explicitly_directional() {
    assert!(FundAmount::new(0).is_err());
    let request = SetFundsRequest::new(
        FundTransaction::Credit,
        FundAmount::new(10_000).unwrap(),
        FundSegment::FuturesAndOptions,
    );
    assert_eq!(request.transaction(), FundTransaction::Credit);
    assert_eq!(request.segment(), FundSegment::FuturesAndOptions);
}

#[test]
fn derivative_instrument_carries_all_contract_identity() {
    let instrument = option();
    assert_eq!(instrument.stock_code(), &stock("NIFTY"));
    assert_eq!(instrument.expiry(), Some(date("2025-02-27")));
    assert_eq!(instrument.right(), Some(OptionRight::Call));
    assert_eq!(instrument.strike(), Some(&money("24000")));
}
