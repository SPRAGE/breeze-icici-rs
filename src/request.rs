use http::Method;
use serde::de::DeserializeOwned;

use crate::error::Error;
use crate::rate_limit::RequestClass;

pub(crate) mod sealed {
    pub trait Sealed {}
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EndpointBase {
    RestV1,
    RestV2,
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthenticationMode {
    SessionExchange,
    SignedV1,
    SessionV2,
}

/// A crate-defined Breeze operation with a typed success response.
///
/// This trait is sealed: applications can pass the SDK's typed requests to
/// [`crate::BreezeClient::execute`], but cannot create arbitrary signed wires
/// or accidentally classify a mutation as a retryable read.
pub trait EndpointRequest: sealed::Sealed + Clone + Send + Sync + 'static {
    type Response: DeserializeOwned + Send + 'static;

    fn operation(&self) -> &'static str;
    fn method(&self) -> Method;
    fn path(&self) -> &'static str;

    #[doc(hidden)]
    fn endpoint_base(&self) -> EndpointBase {
        EndpointBase::RestV1
    }
    #[doc(hidden)]
    fn authentication(&self) -> AuthenticationMode {
        AuthenticationMode::SignedV1
    }
    #[doc(hidden)]
    fn body(&self) -> Result<Vec<u8>, Error>;
    #[doc(hidden)]
    fn query(&self) -> Vec<(String, String)> {
        Vec::new()
    }
    #[doc(hidden)]
    fn request_class(&self) -> RequestClass {
        RequestClass::Read
    }
}

pub(crate) fn compact_json<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, Error> {
    serde_json::to_vec(value)
        .map_err(|error| Error::protocol(format!("request serialization failed: {error}")))
}
