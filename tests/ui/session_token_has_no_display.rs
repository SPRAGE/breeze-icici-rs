use std::fmt::Display;

use breeze_icici::auth::SessionToken;

fn requires_display(_: impl Display) {}

fn main() {
    let token = SessionToken::new("session-token-test").unwrap();
    requires_display(token);
}
