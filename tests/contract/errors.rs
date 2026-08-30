use breeze_icici::account::GetFunds;
use breeze_icici::error::{Error, TimeoutPhase};
use breeze_icici::testing::decode_response;
use http::{HeaderMap, HeaderValue, StatusCode};
use serde_json::Value;

use crate::rest_wire::order_list;
use crate::support::{SECRET_KEY, SESSION_TOKEN, fixture};

fn error_fixture(name: &str) -> (StatusCode, HeaderMap, Vec<u8>) {
    let fixtures: Value = fixture("errors.json");
    let fixture = &fixtures[name];
    let status = StatusCode::from_u16(fixture["http_status"].as_u64().unwrap() as u16).unwrap();
    let mut headers = HeaderMap::new();
    if let Some(values) = fixture.get("headers").and_then(Value::as_object) {
        for (name, value) in values {
            headers.insert(
                http::header::HeaderName::try_from(name).unwrap(),
                HeaderValue::from_str(value.as_str().unwrap()).unwrap(),
            );
        }
    }
    (
        status,
        headers,
        serde_json::to_vec(&fixture["body"]).unwrap(),
    )
}

fn decode_error(name: &str) -> Error {
    let (status, headers, body) = error_fixture(name);
    decode_response(&GetFunds, status, &headers, &body).unwrap_err()
}

#[test]
fn http_and_application_failures_have_actionable_stable_categories() {
    assert!(matches!(
        decode_error("application_error_with_http_200"),
        Error::Api { .. }
    ));
    assert!(matches!(decode_error("bad_request"), Error::Validation(_)));
    assert!(matches!(
        decode_error("unauthorized"),
        Error::Authentication { .. }
    ));
    assert!(matches!(
        decode_error("forbidden"),
        Error::PermissionDenied { .. }
    ));
    assert!(matches!(decode_error("not_found"), Error::NotFound { .. }));
    assert!(matches!(
        decode_error("request_timeout"),
        Error::Timeout {
            phase: TimeoutPhase::Server,
            ..
        }
    ));
    assert!(matches!(decode_error("server_error"), Error::Api { .. }));
    assert!(matches!(decode_error("unknown_status"), Error::Api { .. }));
}

#[test]
fn retry_after_is_parsed_for_rate_limit_errors() {
    let error = decode_error("rate_limited");
    match error {
        Error::RateLimited { retry_after, .. } => {
            assert_eq!(retry_after.unwrap().as_secs(), 2);
        }
        other => panic!("expected rate limit error, got {other:?}"),
    }
}

#[test]
fn success_null_without_an_error_is_a_protocol_failure() {
    assert!(matches!(
        decode_error("success_null"),
        Error::Protocol { .. }
    ));
}

#[test]
fn empty_success_list_is_a_success_for_list_endpoints() {
    let (status, headers, body) = error_fixture("empty_success_list");
    let orders = decode_response(&order_list(), status, &headers, &body).unwrap();
    assert!(orders.is_empty());

    let funds_result = decode_response(&GetFunds, status, &headers, &body);
    assert!(
        matches!(funds_result, Err(Error::Decode { .. })),
        "empty list is valid JSON/envelope but the wrong endpoint success shape"
    );
}

#[test]
fn malformed_and_oversized_bodies_are_bounded_protocol_errors() {
    let malformed = decode_response(
        &GetFunds,
        StatusCode::OK,
        &HeaderMap::new(),
        b"<html>upstream failure</html>",
    )
    .unwrap_err();
    assert!(matches!(malformed, Error::Protocol { .. }));

    let oversized = vec![b'x'; 2 * 1024 * 1024];
    let error =
        decode_response(&GetFunds, StatusCode::OK, &HeaderMap::new(), &oversized).unwrap_err();
    assert!(matches!(error, Error::Protocol { .. }));
    assert!(format!("{error:?}").len() < 4_096);
}

#[test]
fn no_error_rendering_contains_credentials_or_unbounded_bodies() {
    for name in [
        "application_error_with_http_200",
        "bad_request",
        "unauthorized",
        "forbidden",
        "rate_limited",
        "server_error",
    ] {
        let rendered = format!("{:?}", decode_error(name));
        assert!(!rendered.contains(SECRET_KEY));
        assert!(!rendered.contains(SESSION_TOKEN));
        assert!(rendered.len() < 4_096);
    }
}
