mod support;

use breeze_icici::market::{OptionChainRequest, Quote, QuoteRequest};
use support::{AnyError, authenticated_client, input_error, instrument_from_env, operation_arg};

enum MarketRequest {
    Quotes(QuoteRequest),
    OptionChain(OptionChainRequest),
}

#[tokio::main]
async fn main() -> Result<(), AnyError> {
    let operation = operation_arg("cargo run --example market_data -- <quotes|option-chain>")?;
    let instrument = instrument_from_env()?;
    let request = match operation.as_str() {
        "quotes" => MarketRequest::Quotes(QuoteRequest::new(instrument)),
        "option-chain" => MarketRequest::OptionChain(OptionChainRequest::try_from(instrument)?),
        _ => return Err(input_error("operation must be quotes or option-chain").into()),
    };
    let client = authenticated_client().await?;
    let quotes = match request {
        MarketRequest::Quotes(request) => client.market_data().quotes(request).await?,
        MarketRequest::OptionChain(request) => client.market_data().option_chain(request).await?,
    };

    print_quotes(&quotes);
    Ok(())
}

fn print_quotes(quotes: &[Quote]) {
    println!("exchange\tstock\texpiry\tright\tstrike\tlast");
    for quote in quotes {
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            quote.exchange_code(),
            quote.stock_code(),
            quote.expiry_date_raw().unwrap_or("-"),
            quote.right_raw().unwrap_or("-"),
            quote
                .strike_price()
                .map(ToString::to_string)
                .unwrap_or_else(|| "-".to_owned()),
            quote.last_price(),
        );
    }
}
