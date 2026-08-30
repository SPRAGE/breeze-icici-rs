use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use breeze_icici::account::{GetDematHoldings, GetFunds};
use breeze_icici::auth::{ApiSession, SessionToken};
use breeze_icici::client::{Authenticated, BreezeClient, RetryPolicy, Timeouts};
use breeze_icici::error::{Error, TimeoutPhase};
use breeze_icici::rate_limit::RateLimitPolicy;
use breeze_icici::testing::{FixedClock, SequenceClock};
use chrono::Duration as ChronoDuration;
use wiremock::matchers::{body_string, header, method, path, query_param};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

use crate::rest_wire::{cancel_order, historical_v2, place_order};
use crate::support::{
    SESSION_TOKEN, client_with_endpoints, credentials, fixed_time, production_shape_test_endpoints,
    response_fixture,
};

#[derive(Clone)]
struct FailOnceThenSuccess {
    calls: Arc<AtomicUsize>,
    first_status: u16,
    success_body: Arc<Vec<u8>>,
}

impl Respond for FailOnceThenSuccess {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            ResponseTemplate::new(self.first_status).set_body_json(serde_json::json!({
                "Success": null,
                "Status": self.first_status,
                "Error": "synthetic transient failure"
            }))
        } else {
            ResponseTemplate::new(200).set_body_bytes((*self.success_body).clone())
        }
    }
}

fn retrying_client(server: &MockServer, clock: Arc<SequenceClock>) -> BreezeClient<Authenticated> {
    BreezeClient::builder(credentials())
        .session_token(SessionToken::new(SESSION_TOKEN).unwrap())
        .clock(clock)
        .endpoints(production_shape_test_endpoints(&server.uri()))
        .retry_policy(
            RetryPolicy::safe_reads()
                .max_attempts(2)
                .base_delay(Duration::ZERO)
                .max_delay(Duration::ZERO),
        )
        .build()
        .unwrap()
}

#[tokio::test]
async fn reqwest_transport_really_sends_a_signed_json_body_on_get() {
    let server = MockServer::start().await;
    let endpoints = production_shape_test_endpoints(&server.uri());
    let client = client_with_endpoints(endpoints);
    let prepared = breeze_icici::testing::prepare(&client, GetFunds).unwrap();

    Mock::given(method("GET"))
        .and(path("/breezeapi/api/v1/funds"))
        .and(body_string("{}"))
        .and(header("content-type", "application/json"))
        .and(header(
            "x-timestamp",
            prepared.header("x-timestamp").unwrap(),
        ))
        .and(header("x-checksum", prepared.header("x-checksum").unwrap()))
        .and(header("x-appkey", prepared.header("x-appkey").unwrap()))
        .and(header(
            "x-sessiontoken",
            prepared.header("x-sessiontoken").unwrap(),
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_bytes(response_fixture("account.get_funds")),
        )
        .expect(1)
        .mount(&server)
        .await;

    let funds = client.execute(GetFunds).await.unwrap();
    assert_eq!(funds.unallocated_balance().to_string(), "87500");
}

#[tokio::test]
async fn delete_also_sends_the_exact_signed_body() {
    let server = MockServer::start().await;
    let client = client_with_endpoints(production_shape_test_endpoints(&server.uri()));
    let request = cancel_order();
    let prepared = breeze_icici::testing::prepare(&client, request.clone()).unwrap();

    Mock::given(method("DELETE"))
        .and(path("/breezeapi/api/v1/order"))
        .and(body_string(
            String::from_utf8(prepared.body().to_vec()).unwrap(),
        ))
        .and(header("x-checksum", prepared.header("x-checksum").unwrap()))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(response_fixture("orders.cancel")))
        .expect(1)
        .mount(&server)
        .await;

    let receipt = client.execute(request).await.unwrap();
    assert_eq!(receipt.order_id().as_str(), "ORDER-TEST-1");
}

#[tokio::test]
async fn historical_v2_uses_query_and_session_headers_not_v1_signing() {
    let server = MockServer::start().await;
    let client = client_with_endpoints(production_shape_test_endpoints(&server.uri()));

    Mock::given(method("GET"))
        .and(path("/api/v2/historicalcharts"))
        .and(query_param("interval", "1minute"))
        .and(query_param("exch_code", "NFO"))
        .and(query_param("stock_code", "NIFTY"))
        .and(header("x-sessiontoken", SESSION_TOKEN))
        .respond_with(
            ResponseTemplate::new(200).set_body_bytes(response_fixture("market.historical_v2")),
        )
        .expect(1)
        .mount(&server)
        .await;

    let bars = client.execute(historical_v2()).await.unwrap();
    assert_eq!(bars.len(), 1);
    let requests = server.received_requests().await.unwrap();
    assert!(requests[0].body.is_empty());
    assert!(!requests[0].headers.contains_key("x-checksum"));
}

#[tokio::test]
async fn safe_read_retry_gets_a_fresh_timestamp_and_signature() {
    let server = MockServer::start().await;
    let calls = Arc::new(AtomicUsize::new(0));
    let responder = FailOnceThenSuccess {
        calls: calls.clone(),
        first_status: 503,
        success_body: Arc::new(response_fixture("account.demat_holdings")),
    };
    Mock::given(method("GET"))
        .and(path("/breezeapi/api/v1/dematholdings"))
        .respond_with(responder)
        .mount(&server)
        .await;

    let clock = Arc::new(SequenceClock::new([
        fixed_time(),
        fixed_time() + ChronoDuration::seconds(1),
    ]));
    let client = retrying_client(&server, clock);
    let holdings = client.execute(GetDematHoldings).await.unwrap();
    assert_eq!(holdings.len(), 1);
    assert_eq!(calls.load(Ordering::SeqCst), 2);

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].body, requests[1].body);
    assert_ne!(
        requests[0].headers.get("x-timestamp"),
        requests[1].headers.get("x-timestamp")
    );
    assert_ne!(
        requests[0].headers.get("x-checksum"),
        requests[1].headers.get("x-checksum")
    );
}

#[tokio::test]
async fn pending_client_exchanges_api_session_and_can_then_execute_signed_calls() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/breezeapi/api/v1/customerdetails"))
        .and(body_string(
            r#"{"SessionToken":"api-session-test","AppKey":"app-key-test"}"#,
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_bytes(response_fixture("auth.customer_details")),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/breezeapi/api/v1/funds"))
        .and(header("x-sessiontoken", crate::support::SESSION_TOKEN))
        .respond_with(
            ResponseTemplate::new(200).set_body_bytes(response_fixture("account.get_funds")),
        )
        .expect(1)
        .mount(&server)
        .await;

    let pending = BreezeClient::builder(credentials())
        .clock(Arc::new(FixedClock::new(fixed_time())))
        .endpoints(production_shape_test_endpoints(&server.uri()))
        .build_pending()
        .unwrap();
    let (authenticated, customer) = pending
        .authenticate(ApiSession::new(crate::support::API_SESSION).unwrap())
        .await
        .unwrap();

    assert_eq!(customer.user_id().as_str(), "USER-TEST");
    assert_eq!(
        authenticated
            .account()
            .funds()
            .await
            .unwrap()
            .unallocated_balance()
            .to_string(),
        "87500"
    );
}

#[tokio::test]
async fn authentication_errors_redact_the_browser_api_session() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/breezeapi/api/v1/customerdetails"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "Success": null,
            "Status": 401,
            "Error": crate::support::API_SESSION
        })))
        .mount(&server)
        .await;
    let pending = BreezeClient::builder(credentials())
        .endpoints(production_shape_test_endpoints(&server.uri()))
        .build_pending()
        .unwrap();

    let error = pending
        .authenticate(ApiSession::new(crate::support::API_SESSION).unwrap())
        .await
        .unwrap_err();
    assert!(!format!("{error:?}").contains(crate::support::API_SESSION));
}

#[tokio::test]
async fn retrying_safe_reads_includes_broker_408_responses() {
    let server = MockServer::start().await;
    let calls = Arc::new(AtomicUsize::new(0));
    Mock::given(method("GET"))
        .and(path("/breezeapi/api/v1/dematholdings"))
        .respond_with(FailOnceThenSuccess {
            calls: calls.clone(),
            first_status: 408,
            success_body: Arc::new(response_fixture("account.demat_holdings")),
        })
        .mount(&server)
        .await;
    let client = retrying_client(
        &server,
        Arc::new(SequenceClock::new([
            fixed_time(),
            fixed_time() + ChronoDuration::seconds(1),
        ])),
    );

    assert_eq!(client.execute(GetDematHoldings).await.unwrap().len(), 1);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn responses_over_the_byte_limit_are_rejected_before_json_decoding() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/breezeapi/api/v1/funds"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![b'x'; 1024 * 1024 + 1]))
        .mount(&server)
        .await;
    let client = client_with_endpoints(production_shape_test_endpoints(&server.uri()));

    assert!(matches!(
        client.execute(GetFunds).await,
        Err(Error::Protocol { .. })
    ));
}

#[tokio::test]
async fn signed_requests_do_not_follow_http_redirects() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/breezeapi/api/v1/funds"))
        .respond_with(
            ResponseTemplate::new(302)
                .insert_header("Location", format!("{}/redirected", server.uri()))
                .set_body_json(serde_json::json!({
                    "Success": null,
                    "Status": 302,
                    "Error": "redirects are forbidden"
                })),
        )
        .mount(&server)
        .await;
    let client = client_with_endpoints(production_shape_test_endpoints(&server.uri()));

    assert!(matches!(
        client.execute(GetFunds).await,
        Err(Error::Api {
            status: Some(302),
            ..
        })
    ));
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].url.path(), "/breezeapi/api/v1/funds");
}

#[tokio::test]
async fn mutation_is_never_retried_on_a_server_or_ambiguous_failure() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/breezeapi/api/v1/order"))
        .respond_with(ResponseTemplate::new(503).set_body_json(serde_json::json!({
            "Success": null,
            "Status": 503,
            "Error": "synthetic ambiguous mutation failure"
        })))
        .mount(&server)
        .await;

    let clock = Arc::new(SequenceClock::new([
        fixed_time(),
        fixed_time() + ChronoDuration::seconds(1),
    ]));
    let client = retrying_client(&server, clock);
    assert!(matches!(
        client.execute(place_order()).await,
        Err(Error::AmbiguousMutation {
            operation: "order",
            ..
        })
    ));
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn mutation_timeout_reports_unknown_outcome_and_requires_reconciliation() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/breezeapi/api/v1/order"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(100))
                .set_body_bytes(response_fixture("orders.place")),
        )
        .mount(&server)
        .await;
    let client = BreezeClient::builder(credentials())
        .session_token(SessionToken::new(SESSION_TOKEN).unwrap())
        .clock(Arc::new(FixedClock::new(fixed_time())))
        .endpoints(production_shape_test_endpoints(&server.uri()))
        .retry_policy(RetryPolicy::safe_reads().max_attempts(3))
        .timeouts(Timeouts::default().with_total(Duration::from_millis(10)))
        .build()
        .unwrap();

    let error = client.execute(place_order()).await.unwrap_err();
    assert!(matches!(
        &error,
        Error::AmbiguousMutation {
            operation: "order",
            ..
        }
    ));
    let message = error.to_string();
    assert!(message.contains("outcome is unknown"));
    assert!(message.contains("reconcile"));
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn total_deadline_is_reported_separately_from_server_status() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/breezeapi/api/v1/funds"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(100))
                .set_body_bytes(response_fixture("account.get_funds")),
        )
        .mount(&server)
        .await;

    let client = BreezeClient::builder(credentials())
        .session_token(SessionToken::new(SESSION_TOKEN).unwrap())
        .clock(Arc::new(FixedClock::new(fixed_time())))
        .endpoints(production_shape_test_endpoints(&server.uri()))
        .timeouts(Timeouts::default().with_total(Duration::from_millis(10)))
        .build()
        .unwrap();

    assert!(matches!(
        client.execute(GetFunds).await,
        Err(Error::Timeout {
            phase: TimeoutPhase::Total,
            ..
        })
    ));
}

#[tokio::test]
async fn first_byte_deadline_has_its_own_error_phase() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/breezeapi/api/v1/funds"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(100))
                .set_body_bytes(response_fixture("account.get_funds")),
        )
        .mount(&server)
        .await;
    let client = BreezeClient::builder(credentials())
        .session_token(SessionToken::new(SESSION_TOKEN).unwrap())
        .endpoints(production_shape_test_endpoints(&server.uri()))
        .timeouts(
            Timeouts::default()
                .with_first_byte(Duration::from_millis(10))
                .with_total(Duration::from_secs(1)),
        )
        .build()
        .unwrap();

    assert!(matches!(
        client.execute(GetFunds).await,
        Err(Error::Timeout {
            phase: TimeoutPhase::FirstByte,
            ..
        })
    ));
}

#[tokio::test]
async fn total_deadline_cancels_a_pending_local_rate_limit_wait() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/breezeapi/api/v1/funds"))
        .respond_with(
            ResponseTemplate::new(200).set_body_bytes(response_fixture("account.get_funds")),
        )
        .expect(1)
        .mount(&server)
        .await;
    let client = BreezeClient::builder(credentials())
        .session_token(SessionToken::new(SESSION_TOKEN).unwrap())
        .endpoints(production_shape_test_endpoints(&server.uri()))
        .rate_limit_policy(RateLimitPolicy::new(1, 5_000, 10).unwrap())
        .timeouts(Timeouts::default().with_total(Duration::from_millis(20)))
        .build()
        .unwrap();

    client.execute(GetFunds).await.unwrap();
    assert!(matches!(
        client.execute(GetFunds).await,
        Err(Error::Timeout {
            phase: TimeoutPhase::Total,
            ..
        })
    ));
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn retry_after_never_pushes_a_safe_read_past_its_total_deadline() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/breezeapi/api/v1/funds"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("Retry-After", "2")
                .set_body_json(serde_json::json!({
                    "Success": null,
                    "Status": 429,
                    "Error": "synthetic rate limit"
                })),
        )
        .expect(1)
        .mount(&server)
        .await;
    let client = BreezeClient::builder(credentials())
        .session_token(SessionToken::new(SESSION_TOKEN).unwrap())
        .endpoints(production_shape_test_endpoints(&server.uri()))
        .retry_policy(
            RetryPolicy::safe_reads()
                .max_attempts(2)
                .base_delay(Duration::ZERO)
                .max_delay(Duration::ZERO),
        )
        .timeouts(Timeouts::default().with_total(Duration::from_millis(20)))
        .build()
        .unwrap();

    assert!(matches!(
        client.execute(GetFunds).await,
        Err(Error::Timeout {
            phase: TimeoutPhase::Total,
            ..
        })
    ));
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[test]
fn insecure_non_loopback_endpoint_override_is_rejected() {
    let endpoints = breeze_icici::client::EndpointSet::builder()
        .rest_v1(
            "http://broker-proxy.example/breezeapi/api/v1/"
                .parse()
                .unwrap(),
        )
        .build();
    assert!(endpoints.is_err());
}

#[test]
fn endpoint_overrides_reject_queries_and_non_directory_paths() {
    let query = breeze_icici::client::EndpointSet::builder()
        .rest_v1(
            "https://example.test/api/v1/?tenant=secret"
                .parse()
                .unwrap(),
        )
        .build();
    assert!(query.is_err());

    let non_directory = breeze_icici::client::EndpointSet::builder()
        .rest_v1("https://example.test/api/v1".parse().unwrap())
        .build();
    assert!(non_directory.is_err());
}
