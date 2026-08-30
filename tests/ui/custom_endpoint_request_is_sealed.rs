use breeze_icici::{EndpointRequest, Error};
use http::Method;

#[derive(Clone)]
struct RawMutation;

impl EndpointRequest for RawMutation {
    type Response = serde_json::Value;

    fn operation(&self) -> &'static str {
        "raw_mutation"
    }

    fn method(&self) -> Method {
        Method::POST
    }

    fn path(&self) -> &'static str {
        "/order"
    }

    fn body(&self) -> Result<Vec<u8>, Error> {
        Ok(br#"{"order_type":"market"}"#.to_vec())
    }
}

fn main() {}
