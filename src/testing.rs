use std::collections::VecDeque;
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use http::{HeaderMap, StatusCode};

use crate::EndpointRequest;
use crate::auth::Credentials;
use crate::client::{Authenticated, BreezeClient};
use crate::clock::Clock;
use crate::error::Error;

pub use crate::client::PreparedRequest;
pub use crate::rate_limit::{RateDecision, RateLimiterModel};

#[derive(Debug)]
pub struct FixedClock {
    value: DateTime<Utc>,
}

impl FixedClock {
    pub fn new(value: DateTime<Utc>) -> Self {
        Self { value }
    }
}

impl Clock for FixedClock {
    fn now(&self) -> DateTime<Utc> {
        self.value
    }
}

#[derive(Debug)]
pub struct SequenceClock {
    values: Mutex<VecDeque<DateTime<Utc>>>,
    last: Mutex<Option<DateTime<Utc>>>,
}

impl SequenceClock {
    pub fn new(values: impl IntoIterator<Item = DateTime<Utc>>) -> Self {
        Self {
            values: Mutex::new(values.into_iter().collect()),
            last: Mutex::new(None),
        }
    }
}

impl Clock for SequenceClock {
    fn now(&self) -> DateTime<Utc> {
        if let Some(value) = self
            .values
            .lock()
            .expect("clock mutex poisoned")
            .pop_front()
        {
            *self.last.lock().expect("clock mutex poisoned") = Some(value);
            value
        } else {
            self.last
                .lock()
                .expect("clock mutex poisoned")
                .expect("sequence clock is empty")
        }
    }
}

pub fn prepare<R: EndpointRequest>(
    client: &BreezeClient<Authenticated>,
    request: R,
) -> Result<PreparedRequest, Error> {
    crate::client::prepare_request(client, request)
}

#[derive(Clone, Debug)]
pub struct TestSignedBody(crate::signing::SignedBody);

impl TestSignedBody {
    pub fn timestamp(&self) -> &str {
        self.0.timestamp()
    }
    pub fn checksum(&self) -> &str {
        self.0.checksum()
    }
    pub fn body(&self) -> &[u8] {
        self.0.body()
    }
}

pub fn sign_v1_body(
    credentials: &Credentials,
    _session_token: &str,
    time: DateTime<Utc>,
    body: &[u8],
) -> Result<TestSignedBody, Error> {
    Ok(TestSignedBody(crate::signing::sign(
        credentials,
        time,
        body,
    )))
}

pub fn decode_response<R: EndpointRequest>(
    request: &R,
    status: StatusCode,
    headers: &HeaderMap,
    body: &[u8],
) -> Result<R::Response, Error> {
    crate::client::response::decode_for(request, status, headers, body)
}

// Streaming fakes are defined in the streaming module and re-exported here.
pub use crate::streaming::testing::{FakeSocketIo, StreamTestClient};
