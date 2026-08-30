use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::{Duration, Instant};

use http::{HeaderMap, HeaderName, HeaderValue};
use tokio::sync::Mutex;
use url::Url;

use crate::EndpointRequest;
use crate::account::{CustomerDetails, CustomerDetailsRequest};
use crate::auth::{ApiSession, Credentials, SessionToken};
pub use crate::clock::Clock;
use crate::clock::SystemClock;
use crate::error::{Error, TimeoutPhase, ValidationError};
use crate::rate_limit::{RateDecision, RateLimitPolicy, RateLimiterModel};
use crate::request::{AuthenticationMode, EndpointBase};
use crate::signing::sign;

pub use crate::services::{
    AccountService, MarketDataService, OrdersService, PortfolioService, RiskService, TradesService,
    TradingService,
};

#[derive(Clone, Copy, Debug, Default)]
pub struct Pending;

#[derive(Clone, Copy, Debug, Default)]
pub struct Authenticated;

#[derive(Clone, Debug)]
pub struct EndpointSet {
    pub(crate) rest_v1: Url,
    pub(crate) rest_v2: Url,
    pub(crate) live_feeds: Url,
    pub(crate) live_stream: Url,
    pub(crate) ohlcv: Url,
}

impl EndpointSet {
    pub fn production() -> Self {
        Self {
            rest_v1: Url::parse("https://api.icicidirect.com/breezeapi/api/v1/")
                .expect("static v1 URL"),
            rest_v2: Url::parse("https://breezeapi.icicidirect.com/api/v2/")
                .expect("static v2 URL"),
            live_feeds: Url::parse("https://livefeeds.icicidirect.com/")
                .expect("static live-feeds URL"),
            live_stream: Url::parse("https://livestream.icicidirect.com/")
                .expect("static live-stream URL"),
            ohlcv: Url::parse("https://breezeapi.icicidirect.com/ohlcvstream/")
                .expect("static OHLCV URL"),
        }
    }

    pub fn builder() -> EndpointSetBuilder {
        EndpointSetBuilder {
            endpoints: Self::production(),
            allow_insecure_loopback: false,
        }
    }
}

impl Default for EndpointSet {
    fn default() -> Self {
        Self::production()
    }
}

#[derive(Clone, Debug)]
pub struct EndpointSetBuilder {
    endpoints: EndpointSet,
    allow_insecure_loopback: bool,
}

impl EndpointSetBuilder {
    pub fn rest_v1(mut self, value: Url) -> Self {
        self.endpoints.rest_v1 = value;
        self
    }
    pub fn rest_v2(mut self, value: Url) -> Self {
        self.endpoints.rest_v2 = value;
        self
    }
    pub fn live_feeds(mut self, value: Url) -> Self {
        self.endpoints.live_feeds = value;
        self
    }
    pub fn live_stream(mut self, value: Url) -> Self {
        self.endpoints.live_stream = value;
        self
    }
    pub fn ohlcv(mut self, value: Url) -> Self {
        self.endpoints.ohlcv = value;
        self
    }

    /// Allows plain HTTP only for a literal loopback host. This is intended for
    /// hermetic integration tests and never disables TLS verification for HTTPS.
    pub fn allow_insecure_loopback_for_tests(mut self) -> Self {
        self.allow_insecure_loopback = true;
        self
    }

    pub fn build(self) -> Result<EndpointSet, ValidationError> {
        for url in [
            &self.endpoints.rest_v1,
            &self.endpoints.rest_v2,
            &self.endpoints.live_feeds,
            &self.endpoints.live_stream,
            &self.endpoints.ohlcv,
        ] {
            validate_endpoint(url, self.allow_insecure_loopback)?;
        }
        Ok(self.endpoints)
    }
}

fn validate_endpoint(url: &Url, allow_loopback: bool) -> Result<(), ValidationError> {
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(ValidationError::new(
            "endpoint URL may not contain userinfo, a query, or a fragment",
        ));
    }
    if !url.path().ends_with('/') {
        return Err(ValidationError::new(
            "endpoint URL path must end with a slash",
        ));
    }
    if url.scheme() == "https" {
        return Ok(());
    }
    let loopback = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1"));
    if allow_loopback && url.scheme() == "http" && loopback {
        return Ok(());
    }
    Err(ValidationError::new("endpoint must use HTTPS"))
}

#[derive(Clone, Copy, Debug)]
pub struct Timeouts {
    connect: Duration,
    first_byte: Duration,
    total: Duration,
}

impl Timeouts {
    pub fn with_total(mut self, total: Duration) -> Self {
        self.total = total;
        self
    }
    pub fn with_connect(mut self, connect: Duration) -> Self {
        self.connect = connect;
        self
    }
    pub fn with_first_byte(mut self, first_byte: Duration) -> Self {
        self.first_byte = first_byte;
        self
    }
    pub fn total(self) -> Duration {
        self.total
    }
}

impl Default for Timeouts {
    fn default() -> Self {
        Self {
            connect: Duration::from_secs(10),
            first_byte: Duration::from_secs(30),
            total: Duration::from_secs(60),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RetryPolicy {
    max_attempts: usize,
    base_delay: Duration,
    max_delay: Duration,
}

impl RetryPolicy {
    pub fn disabled() -> Self {
        Self {
            max_attempts: 1,
            base_delay: Duration::ZERO,
            max_delay: Duration::ZERO,
        }
    }

    pub fn safe_reads() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(2),
        }
    }

    pub fn max_attempts(mut self, value: usize) -> Self {
        self.max_attempts = value.max(1);
        self
    }
    pub fn base_delay(mut self, value: Duration) -> Self {
        self.base_delay = value;
        self
    }
    pub fn max_delay(mut self, value: Duration) -> Self {
        self.max_delay = value;
        self
    }

    fn delay(self, retry_index: usize) -> Duration {
        let factor = 1_u32
            .checked_shl(retry_index.min(31) as u32)
            .unwrap_or(u32::MAX);
        self.base_delay.saturating_mul(factor).min(self.max_delay)
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self::disabled()
    }
}

struct ClientInner {
    credentials: Credentials,
    session_token: Option<SessionToken>,
    endpoints: EndpointSet,
    clock: Arc<dyn Clock>,
    retry_policy: RetryPolicy,
    timeouts: Timeouts,
    http: reqwest::Client,
    limiter: Mutex<RateLimiterModel>,
    rate_limit_policy: RateLimitPolicy,
    started: Instant,
}

/// Cloneable async client. The typestate controls whether signed services and
/// trading operations are available.
#[derive(Clone)]
pub struct BreezeClient<State> {
    inner: Arc<ClientInner>,
    state: PhantomData<State>,
}

impl<State> fmt::Debug for BreezeClient<State> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BreezeClient")
            .field("credentials", &"[REDACTED]")
            .field("authenticated", &self.inner.session_token.is_some())
            .finish_non_exhaustive()
    }
}

impl BreezeClient<Pending> {
    pub fn builder(credentials: Credentials) -> BreezeClientBuilder {
        BreezeClientBuilder {
            credentials,
            session_token: None,
            endpoints: EndpointSet::production(),
            clock: Arc::new(SystemClock),
            retry_policy: RetryPolicy::disabled(),
            timeouts: Timeouts::default(),
            rate_limit_policy: RateLimitPolicy::documented_defaults(),
        }
    }

    /// Exchanges a browser-returned API session for an authenticated client.
    pub async fn authenticate(
        self,
        api_session: ApiSession,
    ) -> Result<(BreezeClient<Authenticated>, CustomerDetails), Error> {
        let api_session_for_redaction = api_session.clone();
        let request =
            CustomerDetailsRequest::new(self.inner.credentials.app_key().clone(), api_session);
        let prepared = prepare_request(&self, request.clone())?;
        let response = send_once(&self.inner, &request, &prepared, self.inner.timeouts.total)
            .await
            .map_err(|error| {
                redact_client_error(&self.inner, error)
                    .redact(&[api_session_for_redaction.expose()])
            })?;
        let token = response
            .session_token()
            .cloned()
            .ok_or_else(|| Error::Authentication {
                message: "CustomerDetails did not return a session token".into(),
            })?;
        let inner = ClientInner {
            credentials: self.inner.credentials.clone(),
            session_token: Some(token),
            endpoints: self.inner.endpoints.clone(),
            clock: self.inner.clock.clone(),
            retry_policy: self.inner.retry_policy,
            timeouts: self.inner.timeouts,
            http: self.inner.http.clone(),
            limiter: Mutex::new(RateLimiterModel::new(self.inner.rate_limit_policy)),
            rate_limit_policy: self.inner.rate_limit_policy,
            started: Instant::now(),
        };
        Ok((
            BreezeClient {
                inner: Arc::new(inner),
                state: PhantomData,
            },
            response,
        ))
    }
}

pub struct BreezeClientBuilder {
    credentials: Credentials,
    session_token: Option<SessionToken>,
    endpoints: EndpointSet,
    clock: Arc<dyn Clock>,
    retry_policy: RetryPolicy,
    timeouts: Timeouts,
    rate_limit_policy: RateLimitPolicy,
}

impl fmt::Debug for BreezeClientBuilder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BreezeClientBuilder")
            .field("credentials", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

impl BreezeClientBuilder {
    pub fn session_token(mut self, value: SessionToken) -> Self {
        self.session_token = Some(value);
        self
    }
    pub fn endpoints(mut self, value: EndpointSet) -> Self {
        self.endpoints = value;
        self
    }
    pub fn clock<C: Clock + 'static>(mut self, value: Arc<C>) -> Self {
        self.clock = value;
        self
    }
    pub fn retry_policy(mut self, value: RetryPolicy) -> Self {
        self.retry_policy = value;
        self
    }
    pub fn timeouts(mut self, value: Timeouts) -> Self {
        self.timeouts = value;
        self
    }
    pub fn rate_limit_policy(mut self, value: RateLimitPolicy) -> Self {
        self.rate_limit_policy = value;
        self
    }

    pub fn build(self) -> Result<BreezeClient<Authenticated>, Error> {
        if self.session_token.is_none() {
            return Err(
                ValidationError::new("an authenticated client requires a session token").into(),
            );
        }
        Ok(BreezeClient {
            inner: Arc::new(self.finish()?),
            state: PhantomData,
        })
    }

    pub fn build_pending(self) -> Result<BreezeClient<Pending>, Error> {
        if self.session_token.is_some() {
            return Err(
                ValidationError::new("pending client must not contain a session token").into(),
            );
        }
        Ok(BreezeClient {
            inner: Arc::new(self.finish()?),
            state: PhantomData,
        })
    }

    fn finish(self) -> Result<ClientInner, Error> {
        let http = reqwest::Client::builder()
            .connect_timeout(self.timeouts.connect)
            .redirect(reqwest::redirect::Policy::none());
        #[cfg(feature = "rustls-tls")]
        let http = http.use_rustls_tls();
        #[cfg(all(not(feature = "rustls-tls"), feature = "native-tls"))]
        let http = http.use_native_tls();
        let http = http.build().map_err(|error| Error::Transport {
            message: error.to_string(),
        })?;
        Ok(ClientInner {
            credentials: self.credentials,
            session_token: self.session_token,
            endpoints: self.endpoints,
            clock: self.clock,
            retry_policy: self.retry_policy,
            timeouts: self.timeouts,
            http,
            limiter: Mutex::new(RateLimiterModel::new(self.rate_limit_policy)),
            rate_limit_policy: self.rate_limit_policy,
            started: Instant::now(),
        })
    }
}

impl BreezeClient<Authenticated> {
    pub fn account(&self) -> AccountService {
        AccountService::new(self.clone())
    }

    pub fn market_data(&self) -> MarketDataService {
        MarketDataService::new(self.clone())
    }

    pub fn orders(&self) -> OrdersService {
        OrdersService::new(self.clone())
    }

    pub fn portfolio(&self) -> PortfolioService {
        PortfolioService::new(self.clone())
    }

    pub fn trades(&self) -> TradesService {
        TradesService::new(self.clone())
    }

    pub fn risk(&self) -> RiskService {
        RiskService::new(self.clone())
    }

    /// Returns the explicit order/GTT mutation facade.
    pub fn trading(&self) -> TradingService {
        TradingService::new(self.clone())
    }

    /// Creates a production Socket.IO streaming client from the authenticated
    /// REST session token.
    #[cfg(feature = "streaming")]
    pub fn streaming(&self) -> Result<crate::streaming::StreamingClient, Error> {
        let token = self
            .inner
            .session_token
            .as_ref()
            .ok_or_else(|| Error::Authentication {
                message: "session token is missing".into(),
            })?;
        Ok(crate::streaming::StreamingClient::new(
            token.stream_credentials()?,
            self.inner.endpoints.live_feeds.clone(),
            self.inner.endpoints.live_stream.clone(),
            self.inner.endpoints.ohlcv.clone(),
        ))
    }

    pub async fn execute<R: EndpointRequest>(&self, request: R) -> Result<R::Response, Error> {
        let execution_started = Instant::now();
        let mut attempt = 0usize;
        loop {
            attempt += 1;
            #[cfg(feature = "tracing")]
            tracing::debug!(
                target: "breeze_icici::rest",
                operation = request.operation(),
                attempt,
                mutation = request.request_class().is_mutation(),
                "Breeze request attempt started"
            );
            let remaining = self
                .inner
                .timeouts
                .total
                .saturating_sub(execution_started.elapsed());
            if remaining.is_zero() {
                return Err(Error::Timeout {
                    phase: TimeoutPhase::Total,
                    message: "total request deadline exceeded".into(),
                });
            }
            tokio::time::timeout(remaining, self.acquire(request.request_class()))
                .await
                .map_err(|_| Error::Timeout {
                    phase: TimeoutPhase::Total,
                    message: "total request deadline exceeded while rate limited".into(),
                })?;
            let prepared = prepare_request(self, request.clone())?;
            let remaining = self
                .inner
                .timeouts
                .total
                .saturating_sub(execution_started.elapsed());
            let result = send_once(&self.inner, &request, &prepared, remaining)
                .await
                .map_err(|error| redact_client_error(&self.inner, error));
            if attempt >= self.inner.retry_policy.max_attempts
                || request.request_class().is_mutation()
                || !is_retryable(&result)
            {
                return if request.request_class().is_mutation() {
                    result.map_err(|error| ambiguous_mutation(request.operation(), error))
                } else {
                    result
                };
            }
            let delay = retry_delay(self.inner.retry_policy, attempt - 1, &result);
            if execution_started.elapsed().saturating_add(delay) >= self.inner.timeouts.total {
                return Err(Error::Timeout {
                    phase: TimeoutPhase::Total,
                    message: "retry delay would exceed total request deadline".into(),
                });
            }
            tokio::time::sleep(delay).await;
        }
    }

    async fn acquire(&self, class: crate::rate_limit::RequestClass) {
        loop {
            let elapsed = self.inner.started.elapsed();
            let decision = self
                .inner
                .limiter
                .lock()
                .await
                .try_acquire_at(class, elapsed);
            match decision {
                RateDecision::Allow => return,
                RateDecision::Wait(wait) => tokio::time::sleep(wait).await,
            }
        }
    }
}

async fn send_once<R: EndpointRequest>(
    inner: &ClientInner,
    request: &R,
    prepared: &PreparedRequest,
    total: Duration,
) -> Result<R::Response, Error> {
    let future = async {
        let send = inner
            .http
            .request(prepared.method.clone(), prepared.url.clone())
            .headers(prepared.headers.clone())
            .body(prepared.body.clone())
            .send();
        let mut response = tokio::time::timeout(inner.timeouts.first_byte, send)
            .await
            .map_err(|_| Error::Timeout {
                phase: TimeoutPhase::FirstByte,
                message: "first-byte deadline exceeded".into(),
            })?
            .map_err(classify_reqwest)?;
        if response
            .content_length()
            .is_some_and(|length| length > response::MAX_RESPONSE_BYTES as u64)
        {
            return Err(Error::protocol(
                "response content length exceeds the configured limit",
            ));
        }
        let status = response.status();
        let headers = response.headers().clone();
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(classify_reqwest)? {
            if bytes.len().saturating_add(chunk.len()) > response::MAX_RESPONSE_BYTES {
                return Err(Error::protocol(
                    "response exceeded the configured byte limit",
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        response::decode_for(request, status, &headers, &bytes)
    };
    tokio::time::timeout(total, future)
        .await
        .map_err(|_| Error::Timeout {
            phase: TimeoutPhase::Total,
            message: "total request deadline exceeded".into(),
        })?
}

fn classify_reqwest(error: reqwest::Error) -> Error {
    if error.is_timeout() {
        Error::Timeout {
            phase: TimeoutPhase::Connect,
            message: "HTTP request timed out".into(),
        }
    } else {
        Error::Transport {
            message: crate::error::bounded(error.to_string()),
        }
    }
}

fn is_retryable<T>(result: &Result<T, Error>) -> bool {
    matches!(
        result,
        Err(Error::Api {
            status: Some(500..=599),
            ..
        }) | Err(Error::Transport { .. })
            | Err(Error::RateLimited { .. })
            | Err(Error::Timeout {
                phase: TimeoutPhase::Connect | TimeoutPhase::FirstByte | TimeoutPhase::Server,
                ..
            })
    )
}

fn ambiguous_mutation(operation: &'static str, error: Error) -> Error {
    if matches!(
        &error,
        Error::Transport { .. }
            | Error::Timeout { .. }
            | Error::Protocol { .. }
            | Error::Decode { .. }
            | Error::Api {
                status: Some(500..=599),
                ..
            }
    ) {
        Error::AmbiguousMutation {
            operation,
            message: crate::error::bounded(error.to_string()),
        }
    } else {
        error
    }
}

fn retry_delay<T>(policy: RetryPolicy, retry_index: usize, result: &Result<T, Error>) -> Duration {
    let policy_delay = policy.delay(retry_index);
    match result {
        Err(Error::RateLimited {
            retry_after: Some(value),
            ..
        }) => policy_delay.max(*value),
        _ => policy_delay,
    }
}

fn redact_client_error(inner: &ClientInner, error: Error) -> Error {
    let mut secrets = vec![
        inner.credentials.app_key().expose(),
        inner.credentials.secret_key().expose(),
    ];
    if let Some(token) = &inner.session_token {
        secrets.push(token.expose());
    }
    error.redact(&secrets)
}

#[derive(Clone, Debug)]
pub struct PreparedRequest {
    method: http::Method,
    url: Url,
    headers: HeaderMap,
    body: Vec<u8>,
}

impl PreparedRequest {
    pub fn method(&self) -> &http::Method {
        &self.method
    }
    pub fn url(&self) -> &Url {
        &self.url
    }
    pub fn body(&self) -> &[u8] {
        &self.body
    }
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).and_then(|value| value.to_str().ok())
    }
}

pub(crate) fn prepare_request<R: EndpointRequest, State>(
    client: &BreezeClient<State>,
    request: R,
) -> Result<PreparedRequest, Error> {
    let mut body = request.body()?;
    let base = match request.endpoint_base() {
        EndpointBase::RestV1 => &client.inner.endpoints.rest_v1,
        EndpointBase::RestV2 => &client.inner.endpoints.rest_v2,
    };
    let mut url = base
        .join(request.path().trim_start_matches('/'))
        .map_err(|error| Error::protocol(format!("invalid endpoint path: {error}")))?;
    if !request.query().is_empty() {
        let mut pairs = url.query_pairs_mut();
        for (key, value) in request.query() {
            pairs.append_pair(&key, &value);
        }
    }

    let mut headers = HeaderMap::new();
    match request.authentication() {
        AuthenticationMode::SessionExchange => {
            headers.insert(
                http::header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
        }
        AuthenticationMode::SignedV1 => {
            let token =
                client
                    .inner
                    .session_token
                    .as_ref()
                    .ok_or_else(|| Error::Authentication {
                        message: "session token is missing".into(),
                    })?;
            let signed = sign(&client.inner.credentials, client.inner.clock.now(), &body);
            headers.insert(
                http::header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            insert(&mut headers, "x-timestamp", signed.timestamp())?;
            insert(&mut headers, "x-checksum", signed.checksum())?;
            insert(
                &mut headers,
                "x-appkey",
                client.inner.credentials.app_key().expose(),
            )?;
            insert(&mut headers, "x-sessiontoken", token.expose())?;
            // The body owned by the signing result is the body handed to the
            // transport; the pre-signing allocation is never independently
            // reserialized or transmitted.
            body = signed.body().to_vec();
        }
        AuthenticationMode::SessionV2 => {
            let token =
                client
                    .inner
                    .session_token
                    .as_ref()
                    .ok_or_else(|| Error::Authentication {
                        message: "session token is missing".into(),
                    })?;
            insert(&mut headers, "x-sessiontoken", token.expose())?;
            insert(
                &mut headers,
                "apikey",
                client.inner.credentials.app_key().expose(),
            )?;
        }
    }
    Ok(PreparedRequest {
        method: request.method(),
        url,
        headers,
        body,
    })
}

fn insert(headers: &mut HeaderMap, name: &'static str, value: &str) -> Result<(), Error> {
    let name = HeaderName::from_static(name);
    let value = HeaderValue::from_str(value).map_err(|_| {
        Error::protocol("credential or signature cannot be represented as an HTTP header")
    })?;
    headers.insert(name, value);
    Ok(())
}

// Shared with the feature-gated public testing facade while remaining private
// in normal builds.
pub(crate) mod response {
    use http::{HeaderMap, StatusCode};
    use serde::Deserialize;
    use serde_json::Value;

    use crate::EndpointRequest;
    use crate::error::{Error, TimeoutPhase, ValidationError};

    pub(crate) const MAX_RESPONSE_BYTES: usize = 1024 * 1024;

    #[derive(Deserialize)]
    struct Envelope {
        #[serde(rename = "Success")]
        success: Value,
        #[serde(rename = "Status")]
        status: u16,
        #[serde(rename = "Error")]
        error: Value,
    }

    pub(crate) fn decode_for<R: EndpointRequest>(
        request: &R,
        status: StatusCode,
        headers: &HeaderMap,
        bytes: &[u8],
    ) -> Result<R::Response, Error> {
        if bytes.len() > MAX_RESPONSE_BYTES {
            return Err(Error::protocol(format!(
                "response exceeded {MAX_RESPONSE_BYTES} bytes"
            )));
        }
        let envelope: Envelope = serde_json::from_slice(bytes)
            .map_err(|_| Error::protocol("response was not a valid Breeze JSON envelope"))?;
        let failed = !status.is_success() || envelope.status != 200 || !envelope.error.is_null();
        if failed {
            let effective = if status.is_success() {
                envelope.status
            } else {
                status.as_u16()
            };
            let message = match envelope.error {
                Value::String(value) => crate::error::bounded(value),
                Value::Null => "Breeze request failed".to_owned(),
                value => crate::error::bounded(value.to_string()),
            };
            return Err(match effective {
                400 => Error::Validation(ValidationError::new(message)),
                401 => Error::Authentication { message },
                403 => Error::PermissionDenied { message },
                404 => Error::NotFound { message },
                408 => Error::Timeout {
                    phase: TimeoutPhase::Server,
                    message,
                },
                429 => Error::RateLimited {
                    retry_after: headers
                        .get(http::header::RETRY_AFTER)
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| v.parse().ok())
                        .map(std::time::Duration::from_secs),
                    message,
                },
                other => Error::api(Some(other), message),
            });
        }
        if envelope.success.is_null() {
            return Err(Error::protocol(
                "successful Breeze envelope contained null Success",
            ));
        }
        serde_json::from_value(envelope.success)
            .map_err(|error| Error::decode(request.operation(), error.to_string()))
    }
}
