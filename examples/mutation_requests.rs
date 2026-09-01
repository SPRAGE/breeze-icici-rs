//! Builds validated mutation requests without authenticating or sending them.

use std::error::Error;
use std::str::FromStr;

use breeze_icici::account::{FundAmount, FundSegment, FundTransaction, SetFundsRequest};
use breeze_icici::domain::{
    DerivativeExchange, Exchange, Instrument, Money, OptionRight, Quantity, StockCode, UserRemark,
};
use breeze_icici::gtt::{
    CancelGttOrderRequest, FreshGttOrder, GttIndexKind, GttLeg, GttLegSet, GttOrderId,
    GttOrderRequest, ModifyGttOrderRequest,
};
use breeze_icici::orders::{
    Action, CancelOrderRequest, ModifyOrderRequest, OrderId, PlaceOrderRequest, SquareOffRequest,
    Validity,
};
use chrono::NaiveDate;

fn main() -> Result<(), Box<dyn Error>> {
    let quantity = Quantity::new(1)?;
    let equity = Instrument::equity(Exchange::Nse, StockCode::new("EXAMPLE")?)?;
    let expiry = NaiveDate::from_ymd_opt(2099, 1, 29).expect("valid example date");
    let option = Instrument::option(
        DerivativeExchange::Nfo,
        StockCode::new("NIFTY")?,
        expiry,
        OptionRight::Call,
        Money::from_str("25000")?,
    )?;

    let _set_funds = SetFundsRequest::new(
        FundTransaction::Credit,
        FundAmount::new(1)?,
        FundSegment::Equity,
    );
    let _place = PlaceOrderRequest::limit(
        equity.clone(),
        Action::Buy,
        quantity,
        Money::from_str("100")?,
    )
    .validity(Validity::Day)
    .user_remark(UserRemark::new("RustExample")?)
    .build()?;

    let order_id = OrderId::new("EXAMPLE-ORDER-ID")?;
    let _modify = ModifyOrderRequest::builder(Exchange::Nse, order_id.clone())
        .price(Money::from_str("101")?)
        .build()?;
    let _cancel = CancelOrderRequest::new(order_id, Exchange::Nse);
    let _square_off =
        SquareOffRequest::limit(equity, Action::Sell, quantity, Money::from_str("101")?)
            .validity(Validity::Day)
            .open_quantity(quantity)
            .build()?;

    let target = GttLeg::target(
        Action::Sell,
        Money::from_str("120")?,
        Money::from_str("115")?,
    )?;
    let stop_loss =
        GttLeg::stop_loss(Action::Sell, Money::from_str("80")?, Money::from_str("85")?)?;
    let legs = GttLegSet::cover_oco(target, stop_loss)?;
    let _place_gtt = GttOrderRequest::cover_oco(
        option,
        quantity,
        FreshGttOrder::limit(Action::Buy, Money::from_str("100")?),
        legs.clone(),
    )
    .index_kind(GttIndexKind::Index)
    .trade_date(NaiveDate::from_ymd_opt(2099, 1, 1).expect("valid example date"))
    .build()?;

    let gtt_id = GttOrderId::new("EXAMPLE-GTT-ID")?;
    let _modify_gtt = ModifyGttOrderRequest::cover_oco(gtt_id.clone(), legs);
    let _cancel_gtt = CancelGttOrderRequest::new(gtt_id);

    let single_leg = GttLeg::stop_loss(
        Action::Buy,
        Money::from_str("100")?,
        Money::from_str("101")?,
    )?;
    let _place_single_gtt = GttOrderRequest::single(
        Instrument::future(DerivativeExchange::Nfo, StockCode::new("NIFTY")?, expiry)?,
        quantity,
        single_leg.clone(),
    )
    .index_kind(GttIndexKind::Index)
    .trade_date(NaiveDate::from_ymd_opt(2099, 1, 1).expect("valid example date"))
    .build()?;
    let _modify_single_gtt =
        ModifyGttOrderRequest::single(GttOrderId::new("EXAMPLE-SINGLE-GTT-ID")?, single_leg);

    println!("Validated mutation requests were constructed; no network request was sent.");
    Ok(())
}
