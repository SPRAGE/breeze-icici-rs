use std::str::FromStr;

use serde_json::Value;

use crate::domain::{Count, Exchange, Money, OptionRight, StockCode};
use crate::instruments::ScriptCode;
use crate::orders::OrderId;

use super::{
    CandleInterval, CandleParts, StreamDecodeError, StreamEvent, candle, commodity, depth,
    depth_level, known, one_click_equity, one_click_fno, order, quote_tick, raw, unknown,
};

pub fn decode_tick(value: &Value) -> Result<StreamEvent, StreamDecodeError> {
    let Some(values) = value.as_array() else {
        return Err(StreamDecodeError::new("tick frame must be an array"));
    };
    let Some(symbol_text) = values.first().and_then(Value::as_str) else {
        return Err(StreamDecodeError::new("tick symbol is missing"));
    };
    let Ok(symbol) = ScriptCode::from_str(symbol_text) else {
        return Ok(StreamEvent::Unknown(raw(value.clone())));
    };
    match (symbol.exchange_qualifier(), symbol.data_kind()) {
        (_, crate::instruments::ScriptDataKind::MarketDepth) => decode_depth(symbol, values),
        (6, crate::instruments::ScriptDataKind::Quotes) => decode_commodity(symbol, values),
        (_, crate::instruments::ScriptDataKind::Quotes) if matches!(values.len(), 21 | 23) => {
            decode_quote(symbol, values)
        }
        _ => Ok(StreamEvent::Unknown(raw(value.clone()))),
    }
}

fn decode_quote(symbol: ScriptCode, values: &[Value]) -> Result<StreamEvent, StreamDecodeError> {
    let exchange = if values.len() == 23 {
        Exchange::Nfo
    } else {
        exchange_from_qualifier(symbol.exchange_qualifier())?
    };
    let last_price = money(at(values, 2)?)?;
    let total_sell_index = if values.len() == 23 { 16 } else { 14 };
    let total_sell_quantity = Some(count(at(values, total_sell_index)?)?);
    let (open_interest, change) = if values.len() == 23 {
        (Some(money(at(values, 12)?)?), Some(money(at(values, 13)?)?))
    } else {
        (None, None)
    };
    Ok(StreamEvent::Quote(quote_tick(
        symbol,
        exchange,
        last_price,
        total_sell_quantity,
        open_interest,
        change,
    )))
}

fn decode_depth(symbol: ScriptCode, values: &[Value]) -> Result<StreamEvent, StreamDecodeError> {
    let rows = at(values, 2)?
        .as_array()
        .ok_or_else(|| StreamDecodeError::new("depth rows must be an array"))?;
    if rows.len() > 100 {
        return Err(StreamDecodeError::new("too many market-depth rows"));
    }
    let bse = symbol.exchange_qualifier() == 1;
    let mut levels = Vec::with_capacity(rows.len());
    for row in rows {
        let row = row
            .as_array()
            .ok_or_else(|| StreamDecodeError::new("depth row must be an array"))?;
        let level = if bse {
            if row.len() != 4 {
                return Err(StreamDecodeError::new(
                    "BSE depth row must contain four fields",
                ));
            }
            depth_level(
                money(&row[0])?,
                count(&row[1])?,
                None,
                money(&row[2])?,
                count(&row[3])?,
                None,
            )
        } else {
            if row.len() < 7 {
                return Err(StreamDecodeError::new("NSE depth row is incomplete"));
            }
            depth_level(
                money(&row[0])?,
                count(&row[1])?,
                Some(integer(&row[2])?),
                money(&row[4])?,
                count(&row[5])?,
                Some(integer(&row[6])?),
            )
        };
        levels.push(level);
    }
    Ok(StreamEvent::MarketDepth(depth(
        symbol,
        if bse { Exchange::Bse } else { Exchange::Nse },
        levels,
    )))
}

fn decode_commodity(
    symbol: ScriptCode,
    values: &[Value],
) -> Result<StreamEvent, StreamDecodeError> {
    if values.len() < 23 {
        return Err(StreamDecodeError::new("commodity frame is incomplete"));
    }
    let raw_depth = values[22]
        .as_array()
        .ok_or_else(|| StreamDecodeError::new("commodity depth is not an array"))?;
    if raw_depth.len() % 8 != 0 || raw_depth.len() > 800 {
        return Err(StreamDecodeError::new("commodity depth shape is invalid"));
    }
    let mut levels = Vec::new();
    for row in raw_depth.chunks(8) {
        levels.push(depth_level(
            money(&row[1])?,
            count(&row[0])?,
            Some(integer(&row[2])?),
            money(&row[5])?,
            count(&row[4])?,
            Some(integer(&row[6])?),
        ));
    }
    Ok(StreamEvent::Commodity(commodity(
        symbol,
        money(&values[5])?,
        money(&values[17])?,
        levels,
    )))
}

pub fn decode_order_notification(
    value: &Value,
) -> Result<super::OrderNotification, StreamDecodeError> {
    let values = value
        .as_array()
        .ok_or_else(|| StreamDecodeError::new("order frame must be an array"))?;
    if values.len() > 256 {
        return Err(StreamDecodeError::new("order frame is too large"));
    }
    let message_type = text(at(values, 11)?)?;
    match message_type {
        "4" | "5" => {
            let action_raw = text(at(values, 15)?)?;
            let status_raw = text(at(values, 20)?)?;
            Ok(order(
                StockCode::new(text(at(values, 14)?)?).map_err(invalid)?,
                action(action_raw),
                status(status_raw),
                OrderId::new(text(at(values, 23)?)?).map_err(invalid)?,
                None,
                None,
            ))
        }
        "6" | "7" => {
            let action_raw = text(at(values, 21)?)?;
            let status_raw = text(at(values, 25)?)?;
            Ok(order(
                StockCode::new(text(at(values, 14)?)?).map_err(invalid)?,
                action(action_raw),
                status(status_raw),
                OrderId::new(text(at(values, 26)?)?).map_err(invalid)?,
                OptionRight::from_wire(text(at(values, 16)?)?),
                Some(money(at(values, 18)?)?),
            ))
        }
        _ => Err(StreamDecodeError::new("unknown order notification layout")),
    }
}

fn action(value: &str) -> super::KnownText {
    match value.to_ascii_lowercase().as_str() {
        "b" | "buy" => known(value, "Buy"),
        "s" | "sell" => known(value, "Sell"),
        _ => unknown(value),
    }
}
fn status(value: &str) -> super::KnownText {
    match value.to_ascii_lowercase().as_str() {
        "o" | "ordered" => known(value, "Ordered"),
        "r" | "requested" => known(value, "Requested"),
        "e" | "executed" => known(value, "Executed"),
        "c" | "cancelled" => known(value, "Cancelled"),
        _ => unknown(value),
    }
}

pub fn decode_one_click_fno(value: &Value) -> Result<super::OneClickFno, StreamDecodeError> {
    let values = value
        .as_array()
        .ok_or_else(|| StreamDecodeError::new("One Click F&O frame must be an array"))?;
    if values.len() != 28 {
        return Err(StreamDecodeError::new(
            "One Click F&O frame must contain 28 fields",
        ));
    }
    Ok(one_click_fno(
        text(&values[2])?.to_owned(),
        StockCode::new(text(&values[8])?).map_err(invalid)?,
        text(&values[27])?.to_owned(),
    ))
}

pub fn decode_one_click_equity(value: &Value) -> Result<super::OneClickEquity, StreamDecodeError> {
    let object = value
        .as_object()
        .ok_or_else(|| StreamDecodeError::new("One Click Equity frame must be an object"))?;
    if object.len() > 128 {
        return Err(StreamDecodeError::new(
            "One Click Equity frame is too large",
        ));
    }
    let get = |name: &str| {
        object
            .get(name)
            .and_then(Value::as_str)
            .ok_or_else(|| StreamDecodeError::new(format!("missing {name}")))
    };
    Ok(one_click_equity(
        StockCode::new(get("stock_code")?).map_err(invalid)?,
        get("subscription_type")?.to_owned(),
        get("iclick_status")?.to_owned(),
    ))
}

pub fn decode_candle(value: &str) -> Result<StreamEvent, StreamDecodeError> {
    if value.len() > 16 * 1024 {
        return Err(StreamDecodeError::new("candle frame is too large"));
    }
    let fields: Vec<_> = value.split(',').collect();
    let exchange = Exchange::from_wire(fields.first().copied().unwrap_or(""))
        .ok_or_else(|| StreamDecodeError::new("unknown candle exchange"))?;
    let stock_code = StockCode::new(fields.get(1).copied().unwrap_or("")).map_err(invalid)?;
    let parts = match fields.len() {
        9 => CandleParts {
            exchange,
            stock_code,
            open: parse_money(fields[2])?,
            high: parse_money(fields[3])?,
            low: parse_money(fields[4])?,
            close: parse_money(fields[5])?,
            volume: parse_count(fields[6])?,
            interval: CandleInterval::from_channel(fields[8])?,
            right: None,
            strike: None,
            open_interest: None,
        },
        13 => CandleParts {
            exchange,
            stock_code,
            strike: Some(parse_money(fields[3])?),
            right: OptionRight::from_wire(fields[4]),
            open: parse_money(fields[5])?,
            high: parse_money(fields[6])?,
            low: parse_money(fields[7])?,
            close: parse_money(fields[8])?,
            volume: parse_count(fields[9])?,
            open_interest: Some(parse_money(fields[10])?),
            interval: CandleInterval::from_channel(fields[12])?,
        },
        11 => CandleParts {
            exchange,
            stock_code,
            open: parse_money(fields[3])?,
            high: parse_money(fields[4])?,
            low: parse_money(fields[5])?,
            close: parse_money(fields[6])?,
            volume: parse_count(fields[7])?,
            open_interest: Some(parse_money(fields[8])?),
            interval: CandleInterval::from_channel(fields[10])?,
            right: None,
            strike: None,
        },
        _ => return Err(StreamDecodeError::new("unknown candle CSV layout")),
    };
    if fields.len() == 13 && parts.right.is_none() {
        return Err(StreamDecodeError::new("option candle right is invalid"));
    }
    Ok(StreamEvent::Candle(candle(parts)))
}

fn exchange_from_qualifier(value: u16) -> Result<Exchange, StreamDecodeError> {
    match value {
        1 => Ok(Exchange::Bse),
        4 => Ok(Exchange::Nse),
        6 => Ok(Exchange::Mcx),
        _ => Err(StreamDecodeError::new("unknown exchange qualifier")),
    }
}
fn at(values: &[Value], index: usize) -> Result<&Value, StreamDecodeError> {
    values
        .get(index)
        .ok_or_else(|| StreamDecodeError::new("stream frame is incomplete"))
}
fn text(value: &Value) -> Result<&str, StreamDecodeError> {
    value
        .as_str()
        .ok_or_else(|| StreamDecodeError::new("expected string field"))
}
fn money(value: &Value) -> Result<Money, StreamDecodeError> {
    match value {
        Value::String(value) => parse_money(value),
        Value::Number(value) => parse_money(&value.to_string()),
        _ => Err(StreamDecodeError::new("expected decimal field")),
    }
}
fn count(value: &Value) -> Result<Count, StreamDecodeError> {
    match value {
        Value::String(value) => parse_count(value),
        Value::Number(value) => value
            .as_u64()
            .map(Count::new)
            .ok_or_else(|| StreamDecodeError::new("expected count field")),
        _ => Err(StreamDecodeError::new("expected count field")),
    }
}
fn integer(value: &Value) -> Result<u64, StreamDecodeError> {
    count(value).map(Count::get)
}
fn parse_money(value: &str) -> Result<Money, StreamDecodeError> {
    Money::from_str(value).map_err(invalid)
}
fn parse_count(value: &str) -> Result<Count, StreamDecodeError> {
    value
        .parse::<u64>()
        .map(Count::new)
        .map_err(|_| StreamDecodeError::new("invalid count field"))
}
fn invalid(error: impl std::fmt::Display) -> StreamDecodeError {
    StreamDecodeError::new(error.to_string())
}
