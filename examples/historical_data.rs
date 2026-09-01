mod support;

use std::io;

use breeze_icici::market::{HistoricalV1Request, HistoricalV2Request};
use support::{
    AnyError, authenticated_client, date_range_from_env, input_error, instrument_from_env,
    interval_from_env, required_env,
};

enum HistoricalRequest {
    V1(HistoricalV1Request),
    V2(HistoricalV2Request),
}

#[tokio::main]
async fn main() -> Result<(), AnyError> {
    let range = date_range_from_env()?;
    let instrument = instrument_from_env()?;
    let interval = interval_from_env()?;
    let request = match required_env("BREEZE_HISTORICAL_API")?
        .to_ascii_lowercase()
        .as_str()
    {
        "v1" => HistoricalRequest::V1(HistoricalV1Request::new(interval, range, instrument)?),
        "v2" => HistoricalRequest::V2(HistoricalV2Request::new(interval, range, instrument)?),
        _ => return Err(input_error("BREEZE_HISTORICAL_API must be v1 or v2").into()),
    };
    let client = authenticated_client().await?;
    let bars = match request {
        HistoricalRequest::V1(request) => client.market_data().historical_v1(request).await?,
        HistoricalRequest::V2(request) => client.market_data().historical(request).await?,
    };

    let stdout = io::stdout();
    let mut writer = csv::Writer::from_writer(stdout.lock());
    writer.write_record([
        "datetime",
        "stock_code",
        "exchange_code",
        "product_type",
        "expiry_date",
        "right",
        "strike_price",
        "open",
        "high",
        "low",
        "close",
        "volume",
        "open_interest",
        "count",
    ])?;

    for bar in bars {
        writer.write_record([
            bar.datetime_raw().to_owned(),
            bar.stock_code().to_string(),
            bar.exchange_code().to_owned(),
            bar.product_type().unwrap_or_default().to_owned(),
            bar.expiry_date_raw().unwrap_or_default().to_owned(),
            bar.right_raw().unwrap_or_default().to_owned(),
            bar.strike_price()
                .map(ToString::to_string)
                .unwrap_or_default(),
            bar.open().to_string(),
            bar.high().to_string(),
            bar.low().to_string(),
            bar.close().to_string(),
            bar.volume().get().to_string(),
            bar.open_interest()
                .map(ToString::to_string)
                .unwrap_or_default(),
            bar.count()
                .map(|value| value.to_string())
                .unwrap_or_default(),
        ])?;
    }
    writer.flush()?;
    Ok(())
}
