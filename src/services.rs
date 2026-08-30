//! Discoverable service facades over [`BreezeClient::execute`].
//!
//! Every method returns the crate's stable [`Error`] categories. Local builder
//! validation happens before these methods are called. Read retries are opt-in
//! on the client; mutation methods never retry automatically.

use crate::account::{
    DematHolding, Funds, GetDematHoldings, GetFunds, GetMarginRequest, Margin, SetFundsReceipt,
    SetFundsRequest,
};
use crate::client::{Authenticated, BreezeClient};
use crate::domain::Exchange;
use crate::error::Error;
use crate::gtt::{
    CancelGttOrderRequest, CancelGttReceipt, GttOrder, GttOrderListRequest, GttOrderRequest,
    GttReceipt, ModifyGttOrderRequest,
};
use crate::market::{
    HistoricalBar, HistoricalV1Request, HistoricalV2Request, OptionChainRequest, Quote,
    QuoteRequest,
};
use crate::orders::{
    CancelOrderRequest, ModifyOrderRequest, Order, OrderDetailRequest, OrderListRequest,
    OrderPreview, OrderReceipt, PlaceOrderRequest, PreviewOrderRequest, SquareOffRequest,
};
use crate::portfolio::{GetPositions, Holding, HoldingsRequest, Position};
use crate::risk::{
    LimitPriceRequest, LimitPriceResult, MarginCalculation, MarginCalculationRequest,
};
use crate::trades::{Trade, TradeDetailRequest, TradeExecution, TradeListRequest};

macro_rules! service {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Debug)]
        pub struct $name {
            client: BreezeClient<Authenticated>,
        }

        impl $name {
            pub(crate) fn new(client: BreezeClient<Authenticated>) -> Self {
                Self { client }
            }

            /// Returns the authenticated client backing this facade.
            pub fn client(&self) -> &BreezeClient<Authenticated> {
                &self.client
            }
        }
    };
}

service!(
    /// Account balances, allocations, holdings, and margin reads.
    AccountService
);
service!(
    /// Quotes, option-chain data, and both historical protocols.
    MarketDataService
);
service!(
    /// Read-only order history, previews, and the GTT book.
    OrdersService
);
service!(
    /// Portfolio holding and open-position reads.
    PortfolioService
);
service!(
    /// Trade-list and execution-detail reads.
    TradesService
);
service!(
    /// Broker-provided margin and limit-price calculators.
    RiskService
);
service!(
    /// Explicit order, square-off, and GTT mutations.
    ///
    /// This facade is intentionally named `trading`; its methods have side
    /// effects at the broker and never opt into the client's read retry policy.
    TradingService
);

impl AccountService {
    /// Returns the account's dematerialized holdings.
    pub async fn demat_holdings(&self) -> Result<Vec<DematHolding>, Error> {
        self.client.execute(GetDematHoldings).await
    }

    /// Returns current fund balances and segment allocations.
    pub async fn funds(&self) -> Result<Funds, Error> {
        self.client.execute(GetFunds).await
    }

    /// Performs the explicit funds-allocation mutation described by `request`.
    pub async fn set_funds(&self, request: SetFundsRequest) -> Result<SetFundsReceipt, Error> {
        self.client.execute(request).await
    }

    /// Returns margin information for one exchange.
    pub async fn margin(&self, exchange: Exchange) -> Result<Margin, Error> {
        self.client.execute(GetMarginRequest::new(exchange)).await
    }
}

impl MarketDataService {
    /// Fetches v1 historical bars using a signed JSON GET body.
    pub async fn historical_v1(
        &self,
        request: HistoricalV1Request,
    ) -> Result<Vec<HistoricalBar>, Error> {
        self.client.execute(request).await
    }

    /// Uses the distinct v2 query/header protocol rather than v1 body signing.
    pub async fn historical(
        &self,
        request: HistoricalV2Request,
    ) -> Result<Vec<HistoricalBar>, Error> {
        self.client.execute(request).await
    }

    /// Fetches the latest quote response for a typed instrument.
    pub async fn quotes(&self, request: QuoteRequest) -> Result<Vec<Quote>, Error> {
        self.client.execute(request).await
    }

    /// Fetches an option chain using its validated two-of-three filter request.
    pub async fn option_chain(&self, request: OptionChainRequest) -> Result<Vec<Quote>, Error> {
        self.client.execute(request).await
    }
}

impl OrdersService {
    /// Returns the documented detail response for one broker order.
    pub async fn detail(&self, request: OrderDetailRequest) -> Result<Vec<Order>, Error> {
        self.client.execute(request).await
    }

    /// Returns orders in the request's validated date window.
    pub async fn list(&self, request: OrderListRequest) -> Result<Vec<Order>, Error> {
        self.client.execute(request).await
    }

    /// Calculates documented charges without placing an order.
    pub async fn preview(&self, request: PreviewOrderRequest) -> Result<OrderPreview, Error> {
        self.client.execute(request).await
    }

    /// Returns the GTT order book in the requested date window.
    pub async fn gtt_orders(&self, request: GttOrderListRequest) -> Result<Vec<GttOrder>, Error> {
        self.client.execute(request).await
    }
}

impl PortfolioService {
    /// Returns filtered portfolio holdings.
    pub async fn holdings(&self, request: HoldingsRequest) -> Result<Vec<Holding>, Error> {
        self.client.execute(request).await
    }

    /// Returns current portfolio positions.
    pub async fn positions(&self) -> Result<Vec<Position>, Error> {
        self.client.execute(GetPositions).await
    }
}

impl TradesService {
    /// Returns trades in the request's date window.
    pub async fn list(&self, request: TradeListRequest) -> Result<Vec<Trade>, Error> {
        self.client.execute(request).await
    }

    /// Returns executions associated with one order.
    pub async fn detail(&self, request: TradeDetailRequest) -> Result<Vec<TradeExecution>, Error> {
        self.client.execute(request).await
    }
}

impl RiskService {
    /// Runs the broker's margin calculator without placing an order.
    pub async fn calculate_margin(
        &self,
        request: MarginCalculationRequest,
    ) -> Result<MarginCalculation, Error> {
        self.client.execute(request).await
    }

    /// Runs the broker's F&O limit-price/quantity calculator.
    pub async fn limit_price(&self, request: LimitPriceRequest) -> Result<LimitPriceResult, Error> {
        self.client.execute(request).await
    }
}

impl TradingService {
    /// Places one explicit limit order. Mutations are never automatically retried.
    pub async fn place(&self, request: PlaceOrderRequest) -> Result<OrderReceipt, Error> {
        self.client.execute(request).await
    }

    /// Modifies the explicitly selected fields of one order.
    pub async fn modify(&self, request: ModifyOrderRequest) -> Result<OrderReceipt, Error> {
        self.client.execute(request).await
    }

    /// Cancels one identified order.
    pub async fn cancel(&self, request: CancelOrderRequest) -> Result<OrderReceipt, Error> {
        self.client.execute(request).await
    }

    /// Submits an explicit limit square-off request.
    pub async fn square_off(&self, request: SquareOffRequest) -> Result<OrderReceipt, Error> {
        self.client.execute(request).await
    }

    /// Places a validated documented single-leg or cover-OCO GTT request.
    pub async fn place_gtt(&self, request: GttOrderRequest) -> Result<GttReceipt, Error> {
        self.client.execute(request).await
    }

    /// Modifies an existing GTT order.
    pub async fn modify_gtt(&self, request: ModifyGttOrderRequest) -> Result<GttReceipt, Error> {
        self.client.execute(request).await
    }

    /// Cancels an existing GTT order.
    pub async fn cancel_gtt(
        &self,
        request: CancelGttOrderRequest,
    ) -> Result<CancelGttReceipt, Error> {
        self.client.execute(request).await
    }
}
