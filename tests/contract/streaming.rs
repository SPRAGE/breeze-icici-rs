use std::str::FromStr;
use std::time::Duration;

use breeze_icici::auth::SessionToken;
use breeze_icici::domain::{Exchange, OptionRight};
use breeze_icici::instruments::ScriptCode;
use breeze_icici::streaming::codec::{
    decode_candle, decode_one_click_equity, decode_one_click_fno, decode_order_notification,
    decode_tick,
};
use breeze_icici::streaming::{
    CandleInterval, StreamError, StreamEvent, StreamKind, Subscription, SubscriptionInsert,
    SubscriptionSet,
};
use breeze_icici::testing::{FakeSocketIo, StreamTestClient};
use proptest::prelude::*;
use serde_json::{Value, json};

use crate::support::{SESSION_TOKEN, stream_fixture};

fn raw(name: &str) -> Value {
    stream_fixture(name)["raw"].clone()
}

#[test]
fn twenty_one_field_nse_quote_layout_decodes_semantically() {
    let event = decode_tick(&raw("quote_nse")).unwrap();
    let StreamEvent::Quote(quote) = event else {
        panic!("expected quote event")
    };
    assert_eq!(quote.symbol().to_string(), "4.1!1001");
    assert_eq!(quote.exchange(), Exchange::Nse);
    assert_eq!(quote.last_price().to_string(), "421.2");
    assert_eq!(quote.total_sell_quantity().unwrap().get(), 370_391);
}

#[test]
fn twenty_three_field_derivative_quote_retains_open_interest() {
    let event = decode_tick(&raw("quote_nfo")).unwrap();
    let StreamEvent::Quote(quote) = event else {
        panic!("expected quote event")
    };
    assert_eq!(quote.open_interest().unwrap().to_string(), "2435175");
    assert_eq!(quote.change_in_open_interest().unwrap().to_string(), "1500");
}

#[test]
fn bse_and_nse_market_depth_layouts_are_distinct() {
    let StreamEvent::MarketDepth(bse) = decode_tick(&raw("market_depth_bse")).unwrap() else {
        panic!("expected BSE market depth")
    };
    assert_eq!(bse.levels().len(), 2);
    assert_eq!(bse.levels()[0].buy_orders(), None);
    assert_eq!(bse.levels()[0].sell_price().to_string(), "420.5");

    let StreamEvent::MarketDepth(nse) = decode_tick(&raw("market_depth_nse")).unwrap() else {
        panic!("expected NSE market depth")
    };
    assert_eq!(nse.levels().len(), 1);
    assert_eq!(nse.levels()[0].buy_orders(), Some(2));
    assert_eq!(nse.levels()[0].sell_orders(), Some(3));
}

#[test]
fn commodity_layout_and_depth_are_not_misclassified_as_nse_quotes() {
    let StreamEvent::Commodity(tick) = decode_tick(&raw("commodity")).unwrap() else {
        panic!("expected commodity event")
    };
    assert_eq!(tick.last_price().to_string(), "350.5");
    assert_eq!(tick.current_open_interest().to_string(), "12500");
    assert_eq!(tick.depth().len(), 1);
}

#[test]
fn cash_and_derivative_order_notifications_use_their_message_type_layouts() {
    let cash = decode_order_notification(&raw("order_cash")).unwrap();
    assert_eq!(cash.stock_code().as_str(), "ITC");
    assert_eq!(cash.action().as_known_str(), Some("Buy"));
    assert_eq!(cash.status().as_known_str(), Some("Ordered"));
    assert_eq!(cash.order_reference().as_str(), "ORDER-TEST-1");

    let derivative = decode_order_notification(&raw("order_derivative")).unwrap();
    assert_eq!(derivative.stock_code().as_str(), "NIFTY");
    assert_eq!(derivative.right(), Some(&OptionRight::Call));
    assert_eq!(derivative.strike().unwrap().to_string(), "2400000");
    assert_eq!(derivative.status().as_known_str(), Some("Requested"));
}

#[test]
fn one_click_fno_and_equity_examples_decode_without_trading_side_effects() {
    let fno = decode_one_click_fno(&raw("one_click_fno")).unwrap();
    assert_eq!(fno.portfolio_id().as_str(), "PORTFOLIO-TEST");
    assert_eq!(fno.underlying().as_str(), "NIFTY");
    assert_eq!(fno.status().as_str(), "active");

    let equity = decode_one_click_equity(&raw("one_click_equity")).unwrap();
    assert_eq!(equity.stock_code().as_str(), "TESTCO");
    assert_eq!(equity.subscription_type().as_str(), "iclick_2_gain");
    assert_eq!(equity.status().as_str(), "open");
}

#[test]
fn all_documented_candle_csv_layouts_and_intervals_decode() {
    let StreamEvent::Candle(equity) =
        decode_candle(raw("candle_equity").as_str().unwrap()).unwrap()
    else {
        panic!("expected equity candle")
    };
    assert_eq!(equity.exchange(), Exchange::Nse);
    assert_eq!(equity.interval(), CandleInterval::OneSecond);
    assert_eq!(equity.open().to_string(), "18687.95");
    assert_eq!(equity.close().to_string(), "18687.95");
    assert_eq!(equity.volume().get(), 0);

    let StreamEvent::Candle(option) =
        decode_candle(raw("candle_option").as_str().unwrap()).unwrap()
    else {
        panic!("expected option candle")
    };
    assert_eq!(option.right(), Some(&OptionRight::Call));
    assert_eq!(option.strike().unwrap().to_string(), "18700.0");
    assert_eq!(option.open_interest().unwrap().to_string(), "7592550");

    let StreamEvent::Candle(future) =
        decode_candle(raw("candle_future").as_str().unwrap()).unwrap()
    else {
        panic!("expected future candle")
    };
    assert_eq!(future.right(), None);
    assert_eq!(future.open_interest().unwrap().to_string(), "11771450");

    for (wire, interval) in [
        ("1SEC", CandleInterval::OneSecond),
        ("1MIN", CandleInterval::OneMinute),
        ("5MIN", CandleInterval::FiveMinutes),
        ("30MIN", CandleInterval::ThirtyMinutes),
    ] {
        assert_eq!(CandleInterval::from_channel(wire).unwrap(), interval);
        assert_eq!(interval.channel(), wire);
    }
}

#[test]
fn unknown_well_formed_frame_is_preserved_not_reinterpreted() {
    let StreamEvent::Unknown(frame) = decode_tick(&raw("unknown")).unwrap() else {
        panic!("expected unknown frame")
    };
    assert_eq!(frame.value(), &raw("unknown"));
}

fn bounded_json() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        (-1_000_000_i64..=1_000_000_i64).prop_map(|value| json!(value)),
        ".{0,32}".prop_map(Value::String),
    ];
    leaf.prop_recursive(4, 64, 8, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..8).prop_map(Value::Array),
            prop::collection::btree_map("[a-zA-Z0-9_]{0,16}", inner, 0..8)
                .prop_map(|map| Value::Object(map.into_iter().collect())),
        ]
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn arbitrary_bounded_json_never_panics_in_any_frame_decoder(value in bounded_json()) {
        let _ = decode_tick(&value);
        let _ = decode_order_notification(&value);
        let _ = decode_one_click_fno(&value);
        let _ = decode_one_click_equity(&value);
        if let Some(text) = value.as_str() {
            let _ = decode_candle(text);
        }
    }
}

#[test]
fn subscription_set_deduplicates_and_enforces_the_documented_two_thousand_cap() {
    let mut subscriptions = SubscriptionSet::with_limit(2_000);
    let first = Subscription::quote(ScriptCode::from_str("4.1!1001").unwrap());
    assert_eq!(
        subscriptions.insert(first.clone()).unwrap(),
        SubscriptionInsert::Added
    );
    assert_eq!(
        subscriptions.insert(first).unwrap(),
        SubscriptionInsert::AlreadyPresent
    );

    for token in 1002..=3000 {
        subscriptions
            .insert(Subscription::quote(
                ScriptCode::from_str(&format!("4.1!{token}")).unwrap(),
            ))
            .unwrap();
    }
    assert_eq!(subscriptions.len(), 2_000);
    assert!(
        subscriptions
            .insert(Subscription::quote(
                ScriptCode::from_str("4.1!3001").unwrap()
            ))
            .is_err()
    );
}

#[tokio::test]
async fn reconnect_reauthenticates_and_replays_each_active_subscription_once() {
    let credentials = SessionToken::new(SESSION_TOKEN)
        .unwrap()
        .stream_credentials()
        .unwrap();
    let fake = FakeSocketIo::new();
    let client = StreamTestClient::builder(credentials)
        .transport(fake.transport())
        .reconnect_backoff(Duration::from_millis(1), Duration::from_millis(4))
        .build();
    let mut stream = client.connect(StreamKind::MarketData).await.unwrap();

    let first = Subscription::quote(ScriptCode::from_str("4.1!1001").unwrap());
    let second = Subscription::market_depth(ScriptCode::from_str("4.2!1001").unwrap());
    stream.subscribe(first.clone()).await.unwrap();
    stream.subscribe(second.clone()).await.unwrap();
    stream.subscribe(first).await.unwrap();

    fake.disconnect().await;
    fake.allow_next_connection().await;
    stream.wait_until_reconnected().await.unwrap();

    let second_connection = fake.connection(1).await;
    assert_eq!(second_connection.auth_count(), 1);
    assert_eq!(second_connection.join_count(&second), 1);
    assert_eq!(second_connection.total_join_count(), 2);
}

#[tokio::test]
async fn unsubscribe_sends_leave_and_removed_subscription_is_not_replayed() {
    let credentials = SessionToken::new(SESSION_TOKEN)
        .unwrap()
        .stream_credentials()
        .unwrap();
    let fake = FakeSocketIo::new();
    let client = StreamTestClient::builder(credentials)
        .transport(fake.transport())
        .build();
    let mut stream = client.connect(StreamKind::MarketData).await.unwrap();
    let removed = Subscription::quote(ScriptCode::from_str("4.1!1001").unwrap());
    let retained = Subscription::market_depth(ScriptCode::from_str("4.2!1001").unwrap());
    stream.subscribe(removed.clone()).await.unwrap();
    stream.subscribe(retained.clone()).await.unwrap();

    assert!(stream.unsubscribe(&removed).await.unwrap());
    assert!(!stream.unsubscribe(&removed).await.unwrap());
    assert_eq!(fake.connection(0).await.leave_count(&removed), 1);
    fake.allow_next_connection().await;
    stream.wait_until_reconnected().await.unwrap();

    let connection = fake.connection(1).await;
    assert_eq!(connection.join_count(&removed), 0);
    assert_eq!(connection.join_count(&retained), 1);
}

#[tokio::test]
async fn stream_family_and_script_data_kind_are_validated_before_join() {
    let credentials = SessionToken::new(SESSION_TOKEN)
        .unwrap()
        .stream_credentials()
        .unwrap();
    let fake = FakeSocketIo::new();
    let client = StreamTestClient::builder(credentials)
        .transport(fake.transport())
        .build();
    let mut stream = client.connect(StreamKind::MarketData).await.unwrap();

    let candle = Subscription::candle(
        ScriptCode::from_str("4.1!1001").unwrap(),
        CandleInterval::OneMinute,
    );
    assert!(matches!(
        stream.subscribe(candle).await,
        Err(StreamError::InvalidSubscription)
    ));
    let mismatched = Subscription::quote(ScriptCode::from_str("4.2!1001").unwrap());
    assert!(matches!(
        stream.subscribe(mismatched).await,
        Err(StreamError::InvalidSubscription)
    ));
    assert_eq!(fake.connection(0).await.total_join_count(), 0);
}

#[tokio::test]
async fn each_candle_connection_has_one_explicit_interval() {
    let credentials = SessionToken::new(SESSION_TOKEN)
        .unwrap()
        .stream_credentials()
        .unwrap();
    let fake = FakeSocketIo::new();
    let client = StreamTestClient::builder(credentials)
        .transport(fake.transport())
        .build();
    let mut stream = client.connect(StreamKind::Candles).await.unwrap();

    stream
        .subscribe(Subscription::candle(
            ScriptCode::from_str("4.1!1001").unwrap(),
            CandleInterval::OneMinute,
        ))
        .await
        .unwrap();
    stream
        .subscribe(Subscription::candle(
            ScriptCode::from_str("4.1!1002").unwrap(),
            CandleInterval::OneMinute,
        ))
        .await
        .unwrap();
    assert!(matches!(
        stream
            .subscribe(Subscription::candle(
                ScriptCode::from_str("4.1!1003").unwrap(),
                CandleInterval::FiveMinutes,
            ))
            .await,
        Err(StreamError::InvalidSubscription)
    ));
}

#[tokio::test]
async fn subscriptions_are_rejected_after_shutdown() {
    let credentials = SessionToken::new(SESSION_TOKEN)
        .unwrap()
        .stream_credentials()
        .unwrap();
    let fake = FakeSocketIo::new();
    let client = StreamTestClient::builder(credentials)
        .transport(fake.transport())
        .build();
    let mut stream = client.connect(StreamKind::MarketData).await.unwrap();
    stream.shutdown().await.unwrap();

    assert!(matches!(
        stream
            .subscribe(Subscription::quote(
                ScriptCode::from_str("4.1!1001").unwrap()
            ))
            .await,
        Err(StreamError::Closed)
    ));
}

#[tokio::test]
async fn order_notification_overflow_is_visible_and_requires_reconciliation() {
    let credentials = SessionToken::new(SESSION_TOKEN)
        .unwrap()
        .stream_credentials()
        .unwrap();
    let fake = FakeSocketIo::new();
    let client = StreamTestClient::builder(credentials)
        .transport(fake.transport())
        .event_capacity(1)
        .build();
    let mut stream = client.connect(StreamKind::Orders).await.unwrap();

    fake.emit("order", raw("order_cash")).await;
    fake.emit("order", raw("order_derivative")).await;

    assert!(matches!(
        stream.next_event().await,
        Some(Ok(StreamEvent::Order(_)))
    ));
    assert!(matches!(
        stream.next_event().await,
        Some(Err(StreamError::LaggedRequiresReconciliation { .. }))
    ));
}

#[tokio::test]
async fn malformed_stream_events_surface_decode_errors_without_closing_the_stream() {
    let credentials = SessionToken::new(SESSION_TOKEN)
        .unwrap()
        .stream_credentials()
        .unwrap();
    let fake = FakeSocketIo::new();
    let client = StreamTestClient::builder(credentials)
        .transport(fake.transport())
        .build();
    let mut stream = client.connect(StreamKind::Orders).await.unwrap();

    fake.emit("order", json!(["incomplete"])).await;
    assert!(matches!(
        stream.next_event().await,
        Some(Err(StreamError::Decode(_)))
    ));
    fake.emit("order", raw("order_cash")).await;
    assert!(matches!(
        stream.next_event().await,
        Some(Ok(StreamEvent::Order(_)))
    ));
}

#[tokio::test]
async fn reconnect_wait_can_be_bounded_by_the_caller() {
    let credentials = SessionToken::new(SESSION_TOKEN)
        .unwrap()
        .stream_credentials()
        .unwrap();
    let fake = FakeSocketIo::new();
    let client = StreamTestClient::builder(credentials)
        .transport(fake.transport())
        .build();
    let mut stream = client.connect(StreamKind::MarketData).await.unwrap();

    assert!(matches!(
        stream
            .wait_until_reconnected_for(Duration::from_millis(5))
            .await,
        Err(StreamError::Connection { .. })
    ));
}

#[tokio::test]
async fn shutdown_sends_leave_and_stops_reconnect_attempts() {
    let credentials = SessionToken::new(SESSION_TOKEN)
        .unwrap()
        .stream_credentials()
        .unwrap();
    let fake = FakeSocketIo::new();
    let client = StreamTestClient::builder(credentials)
        .transport(fake.transport())
        .build();
    let mut stream = client.connect(StreamKind::MarketData).await.unwrap();
    let subscription = Subscription::quote(ScriptCode::from_str("4.1!1001").unwrap());
    stream.subscribe(subscription.clone()).await.unwrap();

    stream.shutdown().await.unwrap();
    assert_eq!(fake.connection(0).await.leave_count(&subscription), 1);
    fake.disconnect().await;
    tokio::time::sleep(Duration::from_millis(10)).await;
    assert_eq!(fake.connection_count().await, 1);
}
