//! HTTP header sensitivity policy shared by recording (request/response) and
//! transaction admission. The privacy contract and the durable record both
//! rely on this single source of truth (HTTP-01).
//!
//! The mandatory set is closed. Sources can additionally mark specific
//! headers as explicitly sensitive at request construction time via
//! [`HttpRequest::sensitive_header`]; those names propagate into response
//! recording and admission as well, so a name like `X-Api-Key` is protected
//! in either direction.

use std::collections::HashSet;

/// Names that must always redacted regardless of source markings. Names are
/// compared ASCII case-insensitively; this constant holds canonical lowercase
/// spellings.
pub(crate) const MANDATORY_SENSITIVE_HEADERS: [&str; 4] = [
    "authorization",
    "proxy-authorization",
    "cookie",
    "set-cookie",
];

pub(crate) fn is_mandatory_sensitive_header(name: &str) -> bool {
    MANDATORY_SENSITIVE_HEADERS
        .iter()
        .any(|required| name.eq_ignore_ascii_case(required))
}

/// Single decision function used by both request and response recording.
///
/// Returns `true` when the header must be structurally redacted: either the
/// name is mandatory, or the source marked the name explicitly sensitive in
/// the request and that name carries forward to the response.
pub(crate) fn must_redact_header(name: &str, explicit: &HashSet<String>) -> bool {
    if is_mandatory_sensitive_header(name) {
        return true;
    }
    explicit
        .iter()
        .any(|candidate| name.eq_ignore_ascii_case(candidate))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mandatory_set_is_closed_and_canonical() {
        assert_eq!(MANDATORY_SENSITIVE_HEADERS.len(), 4);
        assert!(MANDATORY_SENSITIVE_HEADERS.contains(&"authorization"));
        assert!(MANDATORY_SENSITIVE_HEADERS.contains(&"proxy-authorization"));
        assert!(MANDATORY_SENSITIVE_HEADERS.contains(&"cookie"));
        assert!(MANDATORY_SENSITIVE_HEADERS.contains(&"set-cookie"));
    }

    #[test]
    fn mandatory_match_is_case_insensitive() {
        assert!(is_mandatory_sensitive_header("Authorization"));
        assert!(is_mandatory_sensitive_header("AUTHORIZATION"));
        assert!(is_mandatory_sensitive_header("Set-Cookie"));
        assert!(is_mandatory_sensitive_header("set-cookie"));
        assert!(!is_mandatory_sensitive_header("x-api-key"));
        assert!(!is_mandatory_sensitive_header(""));
    }

    #[test]
    fn explicit_set_overrides_record_decision() {
        let mut explicit = HashSet::new();
        explicit.insert("x-api-key".to_owned());
        assert!(must_redact_header("X-Api-Key", &explicit));
        assert!(must_redact_header("x-api-key", &explicit));
        assert!(!must_redact_header("Accept", &explicit));
    }

    #[test]
    fn mandatory_decision_independent_of_explicit_set() {
        let explicit = HashSet::new();
        assert!(must_redact_header("Cookie", &explicit));
        assert!(must_redact_header("Authorization", &explicit));
        assert!(!must_redact_header("X-Forwarded-For", &explicit));
    }
}
