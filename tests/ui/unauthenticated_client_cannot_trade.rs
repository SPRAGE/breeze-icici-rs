use breeze_icici::auth::{AppKey, Credentials, SecretKey};
use breeze_icici::client::BreezeClient;

fn main() {
    let credentials = Credentials::new(
        AppKey::new("app-key-test").unwrap(),
        SecretKey::new("secret-key-test").unwrap(),
    );
    let pending = BreezeClient::builder(credentials).build_pending().unwrap();
    let _trading = pending.trading();
}
