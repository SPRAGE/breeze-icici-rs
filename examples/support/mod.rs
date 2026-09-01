#![allow(dead_code)]

use std::env;
use std::error::Error as StdError;
use std::io;
use std::str::FromStr;

use breeze_icici::domain::{
    DateRange, DerivativeExchange, Exchange, Instrument, Interval, Money, OptionRight, Quantity,
    StockCode,
};
use breeze_icici::orders::Action;
use breeze_icici::{
    ApiSession, AppKey, Authenticated, BreezeClient, Credentials, SecretKey, SessionToken,
};
use chrono::{DateTime, NaiveDate, Utc};

pub type AnyError = Box<dyn StdError + Send + Sync + 'static>;

pub fn input_error(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

pub fn required_env(name: &str) -> Result<String, AnyError> {
    match env::var(name) {
        Ok(value) if !value.is_empty() => Ok(value),
        _ => Err(input_error(format!("set {name} before running this example")).into()),
    }
}

pub fn optional_env(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.is_empty())
}

pub fn credentials_from_env() -> Result<Credentials, AnyError> {
    Ok(Credentials::new(
        AppKey::new(required_env("BREEZE_APP_KEY")?)?,
        SecretKey::new(required_env("BREEZE_SECRET_KEY")?)?,
    ))
}

/// Restores a session token when one is supplied; otherwise performs the
/// documented CustomerDetails exchange with `BREEZE_API_SESSION`.
pub async fn authenticated_client() -> Result<BreezeClient<Authenticated>, AnyError> {
    let credentials = credentials_from_env()?;
    if let Some(token) = optional_env("BREEZE_SESSION_TOKEN") {
        return Ok(BreezeClient::builder(credentials)
            .session_token(SessionToken::new(token)?)
            .build()?);
    }

    let api_session = ApiSession::new(required_env("BREEZE_API_SESSION")?)?;
    let pending = BreezeClient::builder(credentials).build_pending()?;
    let (client, _) = pending.authenticate(api_session).await?;
    Ok(client)
}

pub fn operation_arg(usage: &str) -> Result<String, AnyError> {
    let mut arguments = env::args().skip(1);
    let operation = arguments
        .next()
        .ok_or_else(|| input_error(format!("usage: {usage}")))?;
    if arguments.next().is_some() {
        return Err(input_error(format!("usage: {usage}")).into());
    }
    Ok(operation)
}

pub fn exchange_from_env() -> Result<Exchange, AnyError> {
    parse_exchange(&required_env("BREEZE_EXCHANGE")?)
}

pub fn parse_exchange(value: &str) -> Result<Exchange, AnyError> {
    match value.to_ascii_uppercase().as_str() {
        "NSE" => Ok(Exchange::Nse),
        "BSE" => Ok(Exchange::Bse),
        "NFO" => Ok(Exchange::Nfo),
        "BFO" => Ok(Exchange::Bfo),
        "NDX" | "CDNSE" => Ok(Exchange::Ndx),
        "MCX" => Ok(Exchange::Mcx),
        _ => Err(input_error("BREEZE_EXCHANGE must be NSE, BSE, NFO, BFO, NDX, or MCX").into()),
    }
}

fn parse_derivative_exchange(value: &str) -> Result<DerivativeExchange, AnyError> {
    match value.to_ascii_uppercase().as_str() {
        "NFO" => Ok(DerivativeExchange::Nfo),
        "BFO" => Ok(DerivativeExchange::Bfo),
        "NDX" | "CDNSE" => Ok(DerivativeExchange::Ndx),
        "MCX" => Ok(DerivativeExchange::Mcx),
        _ => Err(input_error("a derivative instrument requires NFO, BFO, NDX, or MCX").into()),
    }
}

pub fn instrument_from_env() -> Result<Instrument, AnyError> {
    let product = required_env("BREEZE_PRODUCT")?.to_ascii_lowercase();
    let exchange = required_env("BREEZE_EXCHANGE")?;
    let stock_code = StockCode::new(required_env("BREEZE_STOCK_CODE")?)?;

    match product.as_str() {
        "cash" | "equity" => Ok(Instrument::equity(parse_exchange(&exchange)?, stock_code)?),
        "future" | "futures" => Ok(Instrument::future(
            parse_derivative_exchange(&exchange)?,
            stock_code,
            expiry_from_env()?,
        )?),
        "option" | "options" => Ok(Instrument::option(
            parse_derivative_exchange(&exchange)?,
            stock_code,
            expiry_from_env()?,
            option_right_from_env()?,
            money_from_env("BREEZE_STRIKE")?,
        )?),
        _ => Err(input_error("BREEZE_PRODUCT must be cash, futures, or options").into()),
    }
}

pub fn expiry_from_env() -> Result<NaiveDate, AnyError> {
    let value = required_env("BREEZE_EXPIRY")?;
    NaiveDate::parse_from_str(&value, "%Y-%m-%d")
        .map_err(|_| input_error("BREEZE_EXPIRY must use YYYY-MM-DD").into())
}

pub fn option_right_from_env() -> Result<OptionRight, AnyError> {
    match required_env("BREEZE_RIGHT")?.to_ascii_lowercase().as_str() {
        "call" | "ce" | "c" => Ok(OptionRight::Call),
        "put" | "pe" | "p" => Ok(OptionRight::Put),
        _ => Err(input_error("BREEZE_RIGHT must be call or put").into()),
    }
}

pub fn money_from_env(name: &str) -> Result<Money, AnyError> {
    Ok(Money::from_str(&required_env(name)?)?)
}

pub fn quantity_from_env(name: &str) -> Result<Quantity, AnyError> {
    let value = required_env(name)?
        .parse::<u64>()
        .map_err(|_| input_error(format!("{name} must be a positive integer")))?;
    Ok(Quantity::new(value)?)
}

pub fn action_from_env() -> Result<Action, AnyError> {
    match required_env("BREEZE_ACTION")?.to_ascii_lowercase().as_str() {
        "buy" => Ok(Action::Buy),
        "sell" => Ok(Action::Sell),
        _ => Err(input_error("BREEZE_ACTION must be buy or sell").into()),
    }
}

pub fn date_range_from_env() -> Result<DateRange, AnyError> {
    let from = datetime_from_env("BREEZE_FROM")?;
    let to = datetime_from_env("BREEZE_TO")?;
    Ok(DateRange::new(from, to)?)
}

fn datetime_from_env(name: &str) -> Result<DateTime<Utc>, AnyError> {
    let value = required_env(name)?;
    DateTime::parse_from_rfc3339(&value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|_| input_error(format!("{name} must be an RFC 3339 timestamp")).into())
}

pub fn interval_from_env() -> Result<Interval, AnyError> {
    match required_env("BREEZE_INTERVAL")?
        .to_ascii_lowercase()
        .as_str()
    {
        "minute" | "1minute" => Ok(Interval::OneMinute),
        "day" | "1day" => Ok(Interval::OneDay),
        _ => Err(input_error("BREEZE_INTERVAL must be minute, 1minute, day, or 1day").into()),
    }
}

pub fn usize_env(name: &str, default: usize) -> Result<usize, AnyError> {
    match optional_env(name) {
        Some(value) => {
            let value = value
                .parse::<usize>()
                .map_err(|_| input_error(format!("{name} must be a positive integer")))?;
            if value == 0 {
                Err(input_error(format!("{name} must be a positive integer")).into())
            } else {
                Ok(value)
            }
        }
        None => Ok(default),
    }
}
