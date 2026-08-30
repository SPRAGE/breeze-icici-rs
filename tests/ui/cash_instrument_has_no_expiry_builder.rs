use breeze_icici::domain::{Exchange, Instrument, StockCode};

fn main() {
    let equity = Instrument::equity(Exchange::Nse, StockCode::new("ITC").unwrap()).unwrap();
    let _invalid = equity.expiry("2025-02-27");
}
