mod support;

use std::str::FromStr;
use std::time::Duration;

use breeze_icici::instruments::ScriptCode;
use breeze_icici::streaming::{CandleInterval, StreamKind, Subscription};
use support::{
    AnyError, authenticated_client, input_error, operation_arg, required_env, usize_env,
};

#[tokio::main]
async fn main() -> Result<(), AnyError> {
    let operation = operation_arg(
        "cargo run --features streaming --example streaming -- \
         <quote|depth|candles|orders|one-click-fno|one-click-equity>",
    )?;
    let (kind, subscription) = match operation.as_str() {
        "quote" => (
            StreamKind::MarketData,
            Some(Subscription::quote(script_from_env()?)),
        ),
        "depth" => (
            StreamKind::MarketData,
            Some(Subscription::market_depth(script_from_env()?)),
        ),
        "candles" => (
            StreamKind::Candles,
            Some(Subscription::candle(
                script_from_env()?,
                candle_interval_from_env()?,
            )),
        ),
        "orders" => (StreamKind::Orders, None),
        "one-click-fno" => (StreamKind::OneClickFno, None),
        "one-click-equity" => (StreamKind::OneClickEquity, None),
        _ => {
            return Err(input_error(
                "operation must be quote, depth, candles, orders, one-click-fno, or \
                 one-click-equity",
            )
            .into());
        }
    };
    let event_count = usize_env("BREEZE_STREAM_EVENTS", 10)?;
    let timeout = Duration::from_secs(usize_env("BREEZE_STREAM_TIMEOUT_SECONDS", 30)? as u64);
    let client = authenticated_client().await?;
    let streams = client.streaming()?;
    let mut feed = streams.connect(kind).await?;
    if let Some(value) = subscription.clone() {
        feed.subscribe(value).await?;
    }
    for _ in 0..event_count {
        match tokio::time::timeout(timeout, feed.next_event()).await {
            Ok(Some(Ok(event))) => println!("{event:?}"),
            Ok(Some(Err(error))) => eprintln!("stream event error: {error}"),
            Ok(None) => break,
            Err(_) => {
                eprintln!("no event received before the configured timeout");
                break;
            }
        }
    }

    if let Some(value) = &subscription {
        feed.unsubscribe(value).await?;
    }
    feed.shutdown().await?;
    Ok(())
}

fn script_from_env() -> Result<ScriptCode, AnyError> {
    Ok(ScriptCode::from_str(&required_env("BREEZE_SCRIPT_CODE")?)?)
}

fn candle_interval_from_env() -> Result<CandleInterval, AnyError> {
    match required_env("BREEZE_CANDLE_INTERVAL")?
        .to_ascii_uppercase()
        .as_str()
    {
        "1SEC" => Ok(CandleInterval::OneSecond),
        "1MIN" => Ok(CandleInterval::OneMinute),
        "5MIN" => Ok(CandleInterval::FiveMinutes),
        "30MIN" => Ok(CandleInterval::ThirtyMinutes),
        _ => Err(input_error("BREEZE_CANDLE_INTERVAL must be 1SEC, 1MIN, 5MIN, or 30MIN").into()),
    }
}
