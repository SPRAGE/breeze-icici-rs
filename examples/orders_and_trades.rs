mod support;

use breeze_icici::gtt::GttOrderListRequest;
use breeze_icici::orders::{OrderDetailRequest, OrderId, OrderListRequest, PreviewOrderRequest};
use breeze_icici::trades::{TradeDetailRequest, TradeListRequest};
use support::{
    AnyError, action_from_env, authenticated_client, date_range_from_env, exchange_from_env,
    input_error, instrument_from_env, money_from_env, operation_arg, quantity_from_env,
    required_env,
};

enum Operation {
    OrderList(OrderListRequest),
    OrderDetail(OrderDetailRequest),
    TradeList(TradeListRequest),
    TradeDetail(TradeDetailRequest),
    GttOrders(GttOrderListRequest),
    Preview(PreviewOrderRequest),
}

#[tokio::main]
async fn main() -> Result<(), AnyError> {
    let operation = operation_arg(
        "cargo run --example orders_and_trades -- \
         <order-list|order-detail|trade-list|trade-detail|gtt-orders|preview>",
    )?;
    let operation = match operation.as_str() {
        "order-list" => Operation::OrderList(OrderListRequest::new(
            exchange_from_env()?,
            date_range_from_env()?,
        )?),
        "order-detail" => Operation::OrderDetail(OrderDetailRequest::new(
            exchange_from_env()?,
            OrderId::new(required_env("BREEZE_ORDER_ID")?)?,
        )),
        "trade-list" => Operation::TradeList(
            TradeListRequest::builder(exchange_from_env()?, date_range_from_env()?).build()?,
        ),
        "trade-detail" => Operation::TradeDetail(TradeDetailRequest::new(
            exchange_from_env()?,
            OrderId::new(required_env("BREEZE_ORDER_ID")?)?,
        )),
        "gtt-orders" => Operation::GttOrders(GttOrderListRequest::new(date_range_from_env()?)?),
        "preview" => Operation::Preview(
            PreviewOrderRequest::limit(
                instrument_from_env()?,
                action_from_env()?,
                quantity_from_env("BREEZE_QUANTITY")?,
                money_from_env("BREEZE_PRICE")?,
            )
            .build()?,
        ),
        _ => {
            return Err(input_error(
                "operation must be order-list, order-detail, trade-list, trade-detail, \
                 gtt-orders, or preview",
            )
            .into());
        }
    };
    let client = authenticated_client().await?;

    match operation {
        Operation::OrderList(request) => {
            for order in client.orders().list(request).await? {
                println!(
                    "{}\t{}\t{}\t{:?}",
                    order.exchange_code(),
                    order.stock_code(),
                    order.order_id().as_str(),
                    order.status(),
                );
            }
        }
        Operation::OrderDetail(request) => {
            for order in client.orders().detail(request).await? {
                println!(
                    "{}\t{}\t{}\t{:?}",
                    order.exchange_code(),
                    order.stock_code(),
                    order.order_id().as_str(),
                    order.status(),
                );
            }
        }
        Operation::TradeList(request) => {
            for trade in client.trades().list(request).await? {
                println!(
                    "{}\t{}\t{}",
                    trade.exchange_code(),
                    trade.stock_code(),
                    trade.order_id().as_str(),
                );
            }
        }
        Operation::TradeDetail(request) => {
            for trade in client.trades().detail(request).await? {
                println!(
                    "{}\t{}\t{}",
                    trade.exchange_code(),
                    trade.stock_code(),
                    trade.trade_id(),
                );
            }
        }
        Operation::GttOrders(request) => {
            for order in client.orders().gtt_orders(request).await? {
                println!(
                    "{}\t{}\t{}",
                    order.exchange_code(),
                    order.stock_code(),
                    order.fresh_order_id().unwrap_or("-"),
                );
            }
        }
        Operation::Preview(request) => {
            let preview = client.orders().preview(request).await?;
            println!("brokerage: {}", preview.brokerage);
            println!("taxes and other charges: {}", preview.total_other_charges);
            println!("total brokerage: {}", preview.total_brokerage);
        }
    }

    Ok(())
}
