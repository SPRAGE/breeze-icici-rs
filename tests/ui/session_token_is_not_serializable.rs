use breeze_icici::auth::SessionToken;

fn main() {
    let token = SessionToken::new("session-token-test").unwrap();
    let _json = serde_json::to_string(&token).unwrap();
}
