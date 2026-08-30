use breeze_icici::auth::{AppKey, Credentials, SecretKey};

fn main() {
    let credentials = Credentials::new(
        AppKey::new("app-key-test").unwrap(),
        SecretKey::new("secret-key-test").unwrap(),
    );
    let _json = serde_json::to_string(&credentials).unwrap();
}
