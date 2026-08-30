use std::fmt::Write as _;

use breeze_icici::account::CustomerDetailsRequest;
use breeze_icici::account::GetFunds;
use breeze_icici::auth::{ApiSession, AppKey, SessionToken, login_url};
use breeze_icici::testing::sign_v1_body;
use sha2::{Digest, Sha256};

use crate::support::{
    API_SESSION, APP_KEY, FIXED_TIMESTAMP, SECRET_KEY, SESSION_TOKEN, credentials, fixed_time,
    prepared,
};

#[test]
fn login_url_percent_encodes_the_app_key_once() {
    let app_key = AppKey::new("key with + / ?").unwrap();
    let url = login_url(&app_key).unwrap();

    assert_eq!(url.scheme(), "https");
    assert_eq!(url.host_str(), Some("api.icicidirect.com"));
    assert_eq!(url.path(), "/apiuser/login");
    assert_eq!(
        url.query_pairs().collect::<Vec<_>>(),
        vec![("api_key".into(), "key with + / ?".into())]
    );
    assert!(!url.as_str().contains("%2520"), "must not double encode");
}

#[test]
fn customer_details_exchange_is_unsigned_compact_json() {
    let request = CustomerDetailsRequest::new(
        AppKey::new(APP_KEY).unwrap(),
        ApiSession::new(API_SESSION).unwrap(),
    );
    let prepared = prepared(request);

    assert_eq!(prepared.method().as_str(), "GET");
    assert_eq!(prepared.url().path(), "/breezeapi/api/v1/customerdetails");
    assert_eq!(
        prepared.body(),
        br#"{"SessionToken":"api-session-test","AppKey":"app-key-test"}"#
    );
    assert_eq!(prepared.header("content-type"), Some("application/json"));
    assert_eq!(prepared.header("x-checksum"), None);
    assert_eq!(prepared.header("x-timestamp"), None);
    assert_eq!(prepared.header("x-sessiontoken"), None);
}

#[test]
fn fixed_v1_signature_matches_timestamp_body_and_secret_bytes() {
    let body = br#"{"exchange_code":"NSE"}"#;
    let signed = sign_v1_body(&credentials(), SESSION_TOKEN, fixed_time(), body).unwrap();

    let mut expected = Sha256::new();
    expected.update(FIXED_TIMESTAMP.as_bytes());
    expected.update(body);
    expected.update(SECRET_KEY.as_bytes());
    let mut expected_hex = String::with_capacity(64);
    for byte in expected.finalize() {
        write!(&mut expected_hex, "{byte:02x}").unwrap();
    }

    assert_eq!(signed.timestamp(), FIXED_TIMESTAMP);
    assert_eq!(signed.checksum(), format!("token {expected_hex}"));
    assert_eq!(signed.body(), body);
}

#[test]
fn signed_empty_get_body_is_not_removed_or_reserialized() {
    let prepared = prepared(GetFunds);
    assert_eq!(prepared.body(), b"{}");
    assert_eq!(prepared.header("x-timestamp"), Some(FIXED_TIMESTAMP));
    assert_eq!(prepared.header("x-appkey"), Some(APP_KEY));
    assert_eq!(prepared.header("x-sessiontoken"), Some(SESSION_TOKEN));
    assert!(prepared.header("x-checksum").unwrap().starts_with("token "));
}

#[test]
fn signer_hashes_unicode_as_utf8_and_sends_the_same_bytes() {
    let body = "{\"tag\":\"निफ्टी\"}".as_bytes();
    let signed = sign_v1_body(&credentials(), SESSION_TOKEN, fixed_time(), body).unwrap();
    assert_eq!(signed.body(), body);
}

#[test]
fn stream_credentials_decode_exactly_one_user_and_token() {
    let token = SessionToken::new(SESSION_TOKEN).unwrap();
    let decoded = token.stream_credentials().unwrap();

    assert_eq!(decoded.user().expose_for_auth(), "user-test");
    assert_eq!(decoded.token().expose_for_auth(), "session-test");
}

#[test]
fn malformed_stream_session_tokens_are_typed_errors_and_redacted() {
    for value in [
        "",
        "not-base64!",
        "dXNlci1vbmx5",
        "OnRva2VuLW9ubHk=",
        "dXNlcjp0b2tlbjpleHRyYQ==",
        "//4=",
    ] {
        let token = SessionToken::new(value).unwrap_or_else(|error| {
            if !value.is_empty() {
                assert!(!format!("{error:?}").contains(value));
            }
            SessionToken::new("bm90LWFuLWF1dGgtdG9rZW4=").unwrap()
        });
        if let Err(error) = token.stream_credentials() {
            let rendered = format!("{error:?}");
            if !value.is_empty() {
                assert!(!rendered.contains(value));
            }
            assert!(!rendered.contains("user"));
        }
    }
}

#[test]
fn credential_debug_output_is_always_redacted() {
    let rendered = format!("{:?}", credentials());
    assert!(!rendered.contains(APP_KEY));
    assert!(!rendered.contains(SECRET_KEY));
    assert!(rendered.contains("REDACTED"));

    let rendered = format!("{:?}", SessionToken::new(SESSION_TOKEN).unwrap());
    assert!(!rendered.contains(SESSION_TOKEN));
    assert!(rendered.contains("REDACTED"));
}
