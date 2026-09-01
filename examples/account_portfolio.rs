mod support;

use breeze_icici::domain::Exchange;
use breeze_icici::portfolio::HoldingsRequest;
use support::{AnyError, authenticated_client, exchange_from_env, input_error, operation_arg};

enum Operation {
    Funds,
    Demat,
    Margin(Exchange),
    Holdings(HoldingsRequest),
    Positions,
}

#[tokio::main]
async fn main() -> Result<(), AnyError> {
    let operation = operation_arg(
        "cargo run --example account_portfolio -- <funds|demat|margin|holdings|positions>",
    )?;
    let operation = match operation.as_str() {
        "funds" => Operation::Funds,
        "demat" => Operation::Demat,
        "margin" => Operation::Margin(exchange_from_env()?),
        "holdings" => Operation::Holdings(HoldingsRequest::builder(exchange_from_env()?).build()?),
        "positions" => Operation::Positions,
        _ => {
            return Err(input_error(
                "operation must be funds, demat, margin, holdings, or positions",
            )
            .into());
        }
    };
    let client = authenticated_client().await?;

    match operation {
        Operation::Funds => {
            let funds = client.account().funds().await?;
            println!("total bank balance: {}", funds.total_bank_balance());
            println!("unallocated balance: {}", funds.unallocated_balance());
            println!("allocated equity: {}", funds.allocated_equity());
            println!("allocated F&O: {}", funds.allocated_fno());
        }
        Operation::Demat => {
            for holding in client.account().demat_holdings().await? {
                println!("{}\t{}", holding.stock_code(), holding.quantity().get());
            }
        }
        Operation::Margin(exchange) => {
            let margin = client.account().margin(exchange).await?;
            println!("cash limit: {}", margin.cash_limit());
            println!("amount allocated: {}", margin.amount_allocated());
            println!("blocked by trade: {}", margin.block_by_trade());
        }
        Operation::Holdings(request) => {
            for holding in client.portfolio().holdings(request).await? {
                let quantity = holding.quantity_raw().unwrap_or("-");
                let average = holding
                    .average_price()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "-".to_owned());
                println!(
                    "{}\t{}\t{}\t{}",
                    holding.exchange_code(),
                    holding.stock_code(),
                    quantity,
                    average,
                );
            }
        }
        Operation::Positions => {
            for position in client.portfolio().positions().await? {
                let quantity = position.quantity_raw().unwrap_or("-");
                let last = position
                    .last_price()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "-".to_owned());
                println!(
                    "{}\t{}\t{}\t{}",
                    position.exchange_code(),
                    position.stock_code(),
                    quantity,
                    last,
                );
            }
        }
    }

    Ok(())
}
