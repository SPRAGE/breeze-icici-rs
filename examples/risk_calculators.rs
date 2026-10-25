mod support;

use std::str::FromStr;

use breeze_icici::domain::{Count, Money};
use breeze_icici::orders::OrderId;
use breeze_icici::risk::{LimitPriceRequest, MarginCalculationRequest, MarginPosition, SourceFlag};
use support::{
    AnyError, action_from_env, authenticated_client, input_error, instrument_from_env,
    money_from_env, operation_arg, optional_env, quantity_from_env, required_env,
};

enum Operation {
    Margin(MarginCalculationRequest),
    LimitPrice(LimitPriceRequest),
}

#[tokio::main]
async fn main() -> Result<(), AnyError> {
    let operation = operation_arg("cargo run --example risk_calculators -- <margin|limit-price>")?;
    let instrument = instrument_from_env()?;
    let action = action_from_env()?;
    let operation = match operation.as_str() {
        "margin" => Operation::Margin(MarginCalculationRequest::new(vec![MarginPosition::new(
            instrument,
            action,
            quantity_from_env("BREEZE_QUANTITY")?,
            money_from_env("BREEZE_PRICE")?,
        )])?),
        "limit-price" => {
            let source_flag = match required_env("BREEZE_SOURCE_FLAG")?
                .to_ascii_uppercase()
                .as_str()
            {
                "P" | "PORTFOLIO" => SourceFlag::Portfolio,
                "O" | "OTHER" => SourceFlag::Other,
                "M" => SourceFlag::M,
                _ => return Err(input_error("BREEZE_SOURCE_FLAG must be P, M, or O").into()),
            };
            let mut builder = LimitPriceRequest::builder(instrument, action)
                .stop_loss_trigger(money_from_env("BREEZE_STOP_LOSS_TRIGGER")?)
                .source_flag(source_flag)
                .fresh_order_limit(money_from_env("BREEZE_FRESH_ORDER_LIMIT")?);
            if let Some(value) = optional_env("BREEZE_LIMIT_RATE") {
                builder = builder.limit_rate(Money::from_str(&value)?);
            }
            if let Some(value) = optional_env("BREEZE_ORDER_ID") {
                builder = builder.order_reference(OrderId::new(value)?);
            }
            if let Some(value) = optional_env("BREEZE_AVAILABLE_QUANTITY") {
                let value = value.parse::<u64>().map_err(|_| {
                    input_error("BREEZE_AVAILABLE_QUANTITY must be a non-negative integer")
                })?;
                builder = builder.available_quantity(Count::new(value));
            }
            Operation::LimitPrice(builder.build()?)
        }
        _ => return Err(input_error("operation must be margin or limit-price").into()),
    };
    let client = authenticated_client().await?;

    match operation {
        Operation::Margin(request) => {
            let result = client.risk().calculate_margin(request).await?;
            println!(
                "span margin required: {}",
                result
                    .span_margin_required
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_owned()),
            );
            println!(
                "non-span margin required: {}",
                result
                    .non_span_margin_required
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_owned()),
            );
        }
        Operation::LimitPrice(request) => {
            let result = client.risk().limit_price(request).await?;
            println!("available quantity: {}", result.available_quantity);
            println!("limit rate: {}", result.limit_rate);
            println!("order margin: {}", result.order_margin);
        }
    }

    Ok(())
}
