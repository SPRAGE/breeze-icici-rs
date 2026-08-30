use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};

use crate::auth::Credentials;
use crate::clock::wire_timestamp;

#[derive(Clone, Debug)]
pub(crate) struct SignedBody {
    timestamp: String,
    checksum: String,
    body: Vec<u8>,
}

impl SignedBody {
    pub(crate) fn timestamp(&self) -> &str {
        &self.timestamp
    }
    pub(crate) fn checksum(&self) -> &str {
        &self.checksum
    }
    pub(crate) fn body(&self) -> &[u8] {
        &self.body
    }
}

pub(crate) fn sign(credentials: &Credentials, timestamp: DateTime<Utc>, body: &[u8]) -> SignedBody {
    let timestamp = wire_timestamp(timestamp);
    let mut hasher = Sha256::new();
    hasher.update(timestamp.as_bytes());
    hasher.update(body);
    hasher.update(credentials.secret_key().expose().as_bytes());
    let checksum = format!("token {:x}", hasher.finalize());
    SignedBody {
        timestamp,
        checksum,
        body: body.to_vec(),
    }
}
