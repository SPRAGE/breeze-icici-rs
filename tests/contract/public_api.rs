use breeze_icici::account::{CustomerDetails, Funds};
use breeze_icici::auth::{Credentials, SecretKey, SessionToken};
use breeze_icici::client::{
    AccountService, Authenticated, BreezeClient, MarketDataService, OrdersService,
    PortfolioService, RiskService, TradesService, TradingService,
};
use breeze_icici::market::{HistoricalBar, Quote};
use breeze_icici::orders::{Order, OrderReceipt};
use breeze_icici::streaming::{StreamEvent, StreamHandle};
use static_assertions::assert_impl_all;

#[test]
fn core_public_types_are_send_and_sync() {
    assert_impl_all!(Credentials: Send, Sync);
    assert_impl_all!(SecretKey: Send, Sync);
    assert_impl_all!(SessionToken: Send, Sync);
    assert_impl_all!(BreezeClient<Authenticated>: Clone, Send, Sync);
    assert_impl_all!(AccountService: Clone, Send, Sync);
    assert_impl_all!(MarketDataService: Clone, Send, Sync);
    assert_impl_all!(OrdersService: Clone, Send, Sync);
    assert_impl_all!(PortfolioService: Clone, Send, Sync);
    assert_impl_all!(TradesService: Clone, Send, Sync);
    assert_impl_all!(RiskService: Clone, Send, Sync);
    assert_impl_all!(TradingService: Clone, Send, Sync);
    assert_impl_all!(CustomerDetails: Send, Sync);
    assert_impl_all!(Funds: Send, Sync);
    assert_impl_all!(HistoricalBar: Send, Sync);
    assert_impl_all!(Quote: Send, Sync);
    assert_impl_all!(Order: Send, Sync);
    assert_impl_all!(OrderReceipt: Send, Sync);
    assert_impl_all!(StreamEvent: Send);
    assert_impl_all!(StreamHandle: Send);
}

#[test]
fn compile_fail_contracts_protect_the_attention_and_secret_boundaries() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/*.rs");
}
