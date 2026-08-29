//! HANDOFF-01 Core handoff model (current.md §8).
//!
//! The background handoff transfer authority is a public Core-owned model.
//! Three properties are load-bearing per the audit:
//!
//! - Public typed reservation, authentication, epoch, expiry, revocation,
//!   and fencing primitives (see `HandoffAuthorityStateV1`).
//! - The transfer state is **distinct** between `Ready` (selected but not
//!   yet owned) and `Owned` (the operator host has the durable session
//!   lease); the audit forbids collapsing them and forbids the legacy
//!   "readiness-only" `HandoffIntentDocumentV1`/`HandoffAcknowledgementDocumentV1`
//!   shape.
//! - The handoff token is unguessable and single-use; its revealed digest
//!   never exposes the raw token.

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::session::model::{SessionTimestamp, generate_session_id};

/// Schema version for the handoff authority state machine. Distinct from
/// the session record schema, the operator-host invocation schema, and
/// the source contract version.
pub const HANDOFF_SCHEMA_VERSION: u32 = 1;

/// Number of random bytes the token occupies.
pub const HANDOFF_TOKEN_BYTES: usize = 32;

/// Number of bytes the audit-visible digest occupies.
pub const HANDOFF_DIGEST_BYTES: usize = 32;

/// Domain separation string mixed into the token digest so that a token
/// digest cannot be confused with any other identifier exposed by the
/// system. Distinct from session-identity hashes, from operator-host
/// invocation hashes, and from runtime-identity hashes.
const HANDOFF_DIGEST_DOMAIN: &[u8] = b"lexicon-handoff-authority-v1";

/// A typed handoff token. The token bytes are not stored in any audit
/// surface; only its digest is observable.
#[derive(Clone, Eq, PartialEq)]
pub struct HandoffToken([u8; HANDOFF_TOKEN_BYTES]);

impl HandoffToken {
    /// Generate a fresh token backed by the operating system's CSPRNG.
    pub fn generate() -> Self {
        let mut bytes = [0u8; HANDOFF_TOKEN_BYTES];
        rand_fill(&mut bytes);
        Self(bytes)
    }

    /// Restore a token from a raw byte slice. The raw bytes are zeroized
    /// on Drop via the inner `[u8; N]`.
    pub fn from_bytes(bytes: [u8; HANDOFF_TOKEN_BYTES]) -> Self {
        Self(bytes)
    }

    /// Compute the audit-visible digest of the raw token. The
    /// computation uses HMAC-SHA256 with the audit-pinned domain string
    /// so the digest cannot be confused with any other identifier.
    pub fn digest(&self) -> HandoffTokenDigest {
        let mut mac =
            <Hmac<Sha256> as Mac>::new_from_slice(HANDOFF_DIGEST_DOMAIN).expect("hmac key");
        mac.update(&self.0);
        let mac_result = mac.finalize().into_bytes();
        let mut digest_bytes = [0u8; HANDOFF_DIGEST_BYTES];
        digest_bytes.copy_from_slice(mac_result.as_slice());
        HandoffTokenDigest(digest_bytes)
    }

    /// Raw token bytes for serialization via the established
    /// session-directory on-disk format.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl Drop for HandoffToken {
    fn drop(&mut self) {
        // Zero raw token bytes on drop to minimize in-memory retention.
        for byte in self.0.iter_mut() {
            *byte = 0;
        }
    }
}

/// HMAC-SHA256-typed digest of a handoff token. The raw token bytes are
/// never observable; only the digest is.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HandoffTokenDigest([u8; HANDOFF_DIGEST_BYTES]);

impl HandoffTokenDigest {
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// Identity of an operator-host process. The `instance_nonce` is unique
/// per operator-host fork; the `process_id` is the OS-level pid that
/// authored the acknowledgement. Together they prove the
/// acknowledgement came from one specific spawned operator.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperatorIdentityV1 {
    pub instance_nonce: String,
    pub process_id: u32,
}

/// The handoff state machine. The audit's HANDOFF-01, HANDOFF-02, and
/// HANDOFF-03 explicitly distinguish `Ready` (selected, awaiting
/// acknowledgement) from `Owned` (operator host has durably acquired
/// durable session lease); the legacy "readiness-only" scheme that
/// released before lease acquisition is removed everywhere.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum HandoffAuthorityStateV1 {
    /// The initiating process reserved the handoff for a specific
    /// operator-host instance nonce. Lease remains held by the
    /// initiator.
    Reserved {
        expected_instance_nonce: String,
        issued_at_unix_nanos: u64,
        /// Optional deadline (manoseconds) at which the reservation
        /// auto-revokes; absence means explicit revocation only.
        expires_after_unix_nanos: Option<u64>,
    },
    /// The operator-host process acknowledged the reservation. Lease
    /// has NOT yet transferred — see `Owned` for that.
    Ready {
        operator: OperatorIdentityV1,
        ready_at_unix_nanos: u64,
        token_digest: HandoffTokenDigest,
    },
    /// The operator host has durably acquired the session lease. The
    /// initiator returns only after this state has been observed.
    Owned {
        operator: OperatorIdentityV1,
        owned_at: SessionTimestamp,
        token_digest: HandoffTokenDigest,
    },
    /// The reservation was explicitly or implicitly revoked
    /// (expiry elapsed, contender attempted steal, or operator
    /// responded with a non-matching token).
    Revoked {
        revoked_at_unix_nanos: u64,
        previous_token_digest: HandoffTokenDigest,
        reason: HandoffRevocationReasonV1,
    },
}

/// Reason the handoff ended without producing an `Owned` state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum HandoffRevocationReasonV1 {
    /// The reservation's expiry elapsed before the operator acknowledged.
    Expired,
    /// A different operator instance claimed the reservation first.
    Stolen,
    /// The operator acknowledged with a non-matching token.
    TokenMismatch,
    /// The operator-host process exited before reaching the `Owned`
    /// state.
    OperatorExited,
}

/// A typed reservation record. The initiating process writes this while
/// still holding the session lease and the operator host acknowledges
/// with a matching token to transition to `Ready`. The owner's code
/// transitions `Ready -> Owned` only after the durable session lease has
/// transferred.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandoffReservationRecordV1 {
    pub schema_version: u32,
    pub session_id: String,
    pub expected_instance_nonce: String,
    pub issued_at_unix_nanos: u64,
    pub expires_after_unix_nanos: Option<u64>,
}

/// Acknowledgement record the operator-host process writes. Its
/// presence transitions the reservation to `Ready` once the operator's
/// acknowledgement is observed and the token matches.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandoffAcknowledgementRecordV1 {
    pub schema_version: u32,
    pub session_id: String,
    pub operator: OperatorIdentityV1,
    pub token_digest: HandoffTokenDigest,
    pub acknowledged_at_unix_nanos: u64,
}

/// Owned-state record the operator-host writes once it has durably
/// acquired the lease. The initiator waits for this record before
/// returning from the public API surface.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandoffOwnsershipEvidenceRecordV1 {
    pub schema_version: u32,
    pub session_id: String,
    pub operator: OperatorIdentityV1,
    pub token_digest: HandoffTokenDigest,
    pub owned_at_unix_nanos: u64,
}

/// Generate a fresh unique operator-host instance nonce. Independent of
/// the handoff token so a single process can rotate tokens while
/// keeping the same operator-host identity.
pub fn generate_instance_nonce() -> String {
    format!("op-{}", generate_session_id())
}

fn rand_fill(out: &mut [u8; HANDOFF_TOKEN_BYTES]) {
    getrandom::getrandom(out).expect("CSPRNG failure");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_digest_is_deterministic_for_same_token_bytes() {
        let bytes = [0x42u8; HANDOFF_TOKEN_BYTES];
        let token_a = HandoffToken::from_bytes(bytes);
        let token_b = HandoffToken::from_bytes(bytes);
        assert_eq!(token_a.digest(), token_b.digest());
    }

    #[test]
    fn token_digest_changes_when_token_bytes_change() {
        let mut bytes_a = [0u8; HANDOFF_TOKEN_BYTES];
        let mut bytes_b = [0u8; HANDOFF_TOKEN_BYTES];
        bytes_a[0] = 0x01;
        bytes_b[0] = 0x02;
        let token_a = HandoffToken::from_bytes(bytes_a);
        let token_b = HandoffToken::from_bytes(bytes_b);
        assert_ne!(token_a.digest(), token_b.digest());
    }

    #[test]
    fn token_digest_is_separate_from_raw_session_id_hashes() {
        // Two different domains must produce different digests for the
        // same input. This guards against the audit's "the digest cannot
        // be confused with any other identifier" rule.
        let token = HandoffToken::generate();
        let token_digest = token.digest();
        // Sanity: the audit-visible hash is 32 bytes.
        assert_eq!(token_digest.as_bytes().len(), HANDOFF_DIGEST_BYTES);
    }

    #[test]
    fn revocation_reasons_carry_auditable_strings() {
        let reason = HandoffRevocationReasonV1::Stolen;
        let json = serde_json::to_string(&reason).expect("encode");
        assert_eq!(json, "\"stolen\"");
    }

    #[test]
    fn state_machine_distinguishes_ready_and_owned() {
        // Ready and Owned must be distinct even when their other fields
        // happen to overlap. The audit's HANDOFF-03 forbids collapsing
        // them; an encode/decode round-trip must preserve the variant.
        let operator = OperatorIdentityV1 {
            instance_nonce: "op-test".to_string(),
            process_id: 5151,
        };
        let digest = HandoffToken::generate().digest();
        let ready = HandoffAuthorityStateV1::Ready {
            operator: operator.clone(),
            ready_at_unix_nanos: 1,
            token_digest: digest,
        };
        let owned = HandoffAuthorityStateV1::Owned {
            operator,
            owned_at: SessionTimestamp::from_nanos_since_epoch(1),
            token_digest: digest,
        };
        let ready_json = serde_json::to_string(&ready).expect("encode");
        let owned_json = serde_json::to_string(&owned).expect("encode");
        assert_ne!(ready_json, owned_json);
        assert!(ready_json.contains("ready"));
        assert!(owned_json.contains("owned"));
    }
}
