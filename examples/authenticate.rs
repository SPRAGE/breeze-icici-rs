mod support;

use breeze_icici::{ApiSession, AppKey, BreezeClient, Credentials, SecretKey, login_url};
use support::{AnyError, optional_env, required_env};

#[tokio::main]
async fn main() -> Result<(), AnyError> {
    let app_key = AppKey::new(required_env("BREEZE_APP_KEY")?)?;
    println!(
        "Open this URL and complete the ICICI login:\n{}",
        login_url(&app_key)?
    );

    let Some(api_session) = optional_env("BREEZE_API_SESSION") else {
        println!("After login, export BREEZE_API_SESSION and run this example again.");
        return Ok(());
    };
    let credentials =
        Credentials::new(app_key, SecretKey::new(required_env("BREEZE_SECRET_KEY")?)?);
    let api_session = ApiSession::new(api_session)?;
    let pending = BreezeClient::builder(credentials).build_pending()?;
    let (_client, customer) = pending.authenticate(api_session).await?;

    println!("Authenticated Breeze user {}", customer.user_id().as_str());
    Ok(())
}
