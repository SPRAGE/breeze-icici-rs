use std::fmt;
use std::time::Duration;

/// The phase in which a request timed out.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum TimeoutPhase {
    Connect,
    FirstByte,
    Total,
    Server,
}

/// A local validation failure. Values that may contain credentials are never
/// included in its message.
#[derive(Clone, Eq, PartialEq)]
pub struct ValidationError {
    message: String,
}

impl ValidationError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: bounded(message.into()),
        }
    }

    /// A stable, human-readable description of the invalid input.
    pub fn message(&self) -> &str {
        &self.message
    }

    pub(crate) fn redact(mut self, secrets: &[&str]) -> Self {
        self.message = redact_text(self.message, secrets);
        self
    }
}

impl fmt::Debug for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ValidationError")
            .field("message", &self.message)
            .finish()
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ValidationError {}

/// Stable public error categories for validation, transport, Breeze envelopes,
/// and untrusted response data.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("validation failed: {0}")]
    Validation(#[from] ValidationError),

    #[error("authentication failed: {message}")]
    Authentication { message: String },

    #[error("permission denied: {message}")]
    PermissionDenied { message: String },

    #[error("resource not found: {message}")]
    NotFound { message: String },

    #[error("request timed out during {phase:?}: {message}")]
    Timeout {
        phase: TimeoutPhase,
        message: String,
    },

    #[error("rate limited: {message}")]
    RateLimited {
        retry_after: Option<Duration>,
        message: String,
    },

    #[error("Breeze API failure: {message}")]
    Api {
        status: Option<u16>,
        message: String,
    },

    #[error("Breeze protocol failure: {message}")]
    Protocol { message: String },

    #[error("could not decode {operation} response: {message}")]
    Decode {
        operation: &'static str,
        message: String,
    },

    #[error("HTTP transport failure: {message}")]
    Transport { message: String },

    #[error(
        "{operation} mutation outcome is unknown; reconcile broker state before retrying: {message}"
    )]
    AmbiguousMutation {
        operation: &'static str,
        message: String,
    },
}

impl Error {
    pub(crate) fn protocol(message: impl Into<String>) -> Self {
        Self::Protocol {
            message: bounded(message.into()),
        }
    }

    pub(crate) fn decode(operation: &'static str, message: impl Into<String>) -> Self {
        Self::Decode {
            operation,
            message: bounded(message.into()),
        }
    }

    pub(crate) fn api(status: Option<u16>, message: impl Into<String>) -> Self {
        Self::Api {
            status,
            message: bounded(message.into()),
        }
    }

    pub(crate) fn redact(self, secrets: &[&str]) -> Self {
        match self {
            Self::Validation(error) => Self::Validation(error.redact(secrets)),
            Self::Authentication { message } => Self::Authentication {
                message: redact_text(message, secrets),
            },
            Self::PermissionDenied { message } => Self::PermissionDenied {
                message: redact_text(message, secrets),
            },
            Self::NotFound { message } => Self::NotFound {
                message: redact_text(message, secrets),
            },
            Self::Timeout { phase, message } => Self::Timeout {
                phase,
                message: redact_text(message, secrets),
            },
            Self::RateLimited {
                retry_after,
                message,
            } => Self::RateLimited {
                retry_after,
                message: redact_text(message, secrets),
            },
            Self::Api { status, message } => Self::Api {
                status,
                message: redact_text(message, secrets),
            },
            Self::Protocol { message } => Self::Protocol {
                message: redact_text(message, secrets),
            },
            Self::Decode { operation, message } => Self::Decode {
                operation,
                message: redact_text(message, secrets),
            },
            Self::Transport { message } => Self::Transport {
                message: redact_text(message, secrets),
            },
            Self::AmbiguousMutation { operation, message } => Self::AmbiguousMutation {
                operation,
                message: redact_text(message, secrets),
            },
        }
    }
}

pub(crate) fn bounded(mut value: String) -> String {
    const MAX_CHARS: usize = 1_024;
    if value.chars().count() > MAX_CHARS {
        value = value.chars().take(MAX_CHARS).collect();
        value.push('…');
    }
    value
}

fn redact_text(mut value: String, secrets: &[&str]) -> String {
    for secret in secrets.iter().copied().filter(|secret| !secret.is_empty()) {
        value = value.replace(secret, "[REDACTED]");
    }
    bounded(value)
}
