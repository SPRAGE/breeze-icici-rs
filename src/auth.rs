use std::fmt;

use base64::Engine as _;
use url::Url;
use zeroize::{Zeroize, Zeroizing};

use crate::error::{Error, ValidationError};

#[derive(Clone)]
struct SecretValue(String);

impl SecretValue {
    fn new(kind: &'static str, value: impl Into<String>) -> Result<Self, ValidationError> {
        let value = value.into();
        if value.is_empty() {
            return Err(ValidationError::new(format!("{kind} must not be empty")));
        }
        Ok(Self(value))
    }

    fn expose(&self) -> &str {
        &self.0
    }
}

impl Drop for SecretValue {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl fmt::Debug for SecretValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

macro_rules! secret_type {
    ($name:ident, $label:literal) => {
        #[derive(Clone)]
        pub struct $name(SecretValue);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, ValidationError> {
                SecretValue::new($label, value).map(Self)
            }

            pub(crate) fn expose(&self) -> &str {
                self.0.expose()
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(concat!(stringify!($name), "([REDACTED])"))
            }
        }
    };
}

secret_type!(AppKey, "app key");
secret_type!(SecretKey, "secret key");
secret_type!(SessionToken, "session token");
secret_type!(ApiSession, "API session");

/// Long-lived application credentials. Its debug representation never exposes
/// either key and it deliberately does not implement `Serialize`.
#[derive(Clone)]
pub struct Credentials {
    app_key: AppKey,
    secret_key: SecretKey,
}

impl Credentials {
    pub fn new(app_key: AppKey, secret_key: SecretKey) -> Self {
        Self {
            app_key,
            secret_key,
        }
    }

    pub fn app_key(&self) -> &AppKey {
        &self.app_key
    }

    pub fn secret_key(&self) -> &SecretKey {
        &self.secret_key
    }
}

impl fmt::Debug for Credentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Credentials")
            .field("app_key", &"[REDACTED]")
            .field("secret_key", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone)]
pub struct StreamCredential(SecretValue);

impl StreamCredential {
    pub fn expose_for_auth(&self) -> &str {
        self.0.expose()
    }
}

impl fmt::Debug for StreamCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("StreamCredential([REDACTED])")
    }
}

#[derive(Clone, Debug)]
pub struct StreamCredentials {
    user: StreamCredential,
    token: StreamCredential,
}

impl StreamCredentials {
    pub fn user(&self) -> &StreamCredential {
        &self.user
    }

    pub fn token(&self) -> &StreamCredential {
        &self.token
    }
}

impl SessionToken {
    pub fn stream_credentials(&self) -> Result<StreamCredentials, Error> {
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(self.expose())
            .map_err(|_| Error::Authentication {
                message: "invalid stream session token".into(),
            })?;
        let decoded =
            Zeroizing::new(
                String::from_utf8(decoded).map_err(|_| Error::Authentication {
                    message: "invalid stream session token".into(),
                })?,
            );
        let mut parts = decoded.split(':');
        let Some(user) = parts.next() else {
            return Err(Error::Authentication {
                message: "invalid stream session token".into(),
            });
        };
        let Some(token) = parts.next() else {
            return Err(Error::Authentication {
                message: "invalid stream session token".into(),
            });
        };
        if user.is_empty() || token.is_empty() || parts.next().is_some() {
            return Err(Error::Authentication {
                message: "invalid stream session token".into(),
            });
        }
        Ok(StreamCredentials {
            user: StreamCredential(SecretValue::new("stream user", user.to_owned())?),
            token: StreamCredential(SecretValue::new("stream token", token.to_owned())?),
        })
    }
}

/// Builds the browser login URL. Only the app key is included, as required by
/// the documented login flow.
pub fn login_url(app_key: &AppKey) -> Result<Url, Error> {
    let mut url = Url::parse("https://api.icicidirect.com/apiuser/login")
        .map_err(|error| Error::protocol(error.to_string()))?;
    url.query_pairs_mut()
        .append_pair("api_key", app_key.expose());
    Ok(url)
}
