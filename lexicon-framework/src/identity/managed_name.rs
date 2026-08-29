// NAME-01 typed safe-name grammar for managed source/project names.
//
// The audit replaces permissive path-oriented name checks with a single
// bounded grammar:
//   * bytes 1..=MAX_NAME_BYTES (63)
//   * lowercase ASCII letters or ASCII digits at both edges
//   * interior characters are lowercase ASCII letters, digits, or '-'
//   * excludes a fixed reserved list
//
// Source and project names parsed through this grammar are safe to embed
// into filesystem paths, TOML interpolations, package names, binary
// names, and Rust templates without leaking path separators or shell
// metacharacters.

use std::fmt;
use std::str::FromStr;

/// Maximum number of bytes allowed in a managed name. The audit's exact
/// value is 63; longer names reject.
pub const MAX_MANAGED_NAME_BYTES: usize = 63;

/// Names that collide with framework layout, source state, or reserved
/// Windows device identifiers. Resolving them here prevents `validation
/// -> scaffold generation -> publication -> build` from producing a
/// `runtime/` directory that the framework will then mistake for its own.
pub const RESERVED_NAMES: &[&str] = &[
    "lexicon",
    "http",
    "data",
    "state",
    "runtime",
    "sessions",
    "get-raw-data",
    "process-data",
    "con",
    "prn",
    "aux",
    "nul",
    "com1",
    "com2",
    "com3",
    "com4",
    "com5",
    "com6",
    "com7",
    "com8",
    "com9",
    "lpt1",
    "lpt2",
    "lpt3",
    "lpt4",
    "lpt5",
    "lpt6",
    "lpt7",
    "lpt8",
    "lpt9",
];

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ManagedName(String);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManagedNameError {
    /// Name was empty after no whitespace stripping.
    Empty,
    /// Name exceeded `MAX_MANAGED_NAME_BYTES`.
    TooLong,
    /// Name violated the grammar (edge byte not lowercase letter or
    /// digit, or interior byte outside the allowed set).
    InvalidGrammar,
    /// Name matched a reserved entry on either platform.
    Reserved,
}

impl ManagedName {
    /// Parse a candidate name into a `ManagedName`, applying the public
    /// grammar: byte length, edge bytes, interior bytes, and the reserved
    /// set. No whitespace stripping is performed; callers can trim first
    /// if they need to.
    pub fn parse(value: &str) -> Result<Self, ManagedNameError> {
        if value.is_empty() {
            return Err(ManagedNameError::Empty);
        }
        if value.len() > MAX_MANAGED_NAME_BYTES {
            return Err(ManagedNameError::TooLong);
        }
        let bytes = value.as_bytes();
        let edge_ok = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();
        if !edge_ok(bytes[0]) || !edge_ok(bytes[bytes.len() - 1]) {
            return Err(ManagedNameError::InvalidGrammar);
        }
        if !bytes
            .iter()
            .all(|byte| edge_ok(*byte) || *byte == b'-')
        {
            return Err(ManagedNameError::InvalidGrammar);
        }
        if RESERVED_NAMES.contains(&value) {
            return Err(ManagedNameError::Reserved);
        }
        Ok(Self(value.to_owned()))
    }

    /// Borrow the inner string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ManagedName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ManagedName {
    type Err = ManagedNameError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserved_set_is_closed() {
        assert!(RESERVED_NAMES.contains(&"lexicon"));
        assert!(RESERVED_NAMES.contains(&"get-raw-data"));
        assert!(RESERVED_NAMES.contains(&"nul"));
        assert!(RESERVED_NAMES.contains(&"com1"));
        assert!(RESERVED_NAMES.contains(&"lpt9"));
    }

    #[test]
    fn empty_and_overlong_names_are_rejected() {
        assert_eq!(ManagedName::parse(""), Err(ManagedNameError::Empty));
        let over = "a".repeat(MAX_MANAGED_NAME_BYTES + 1);
        assert_eq!(ManagedName::parse(&over), Err(ManagedNameError::TooLong));
    }

    #[test]
    fn edge_byte_must_be_lowercase_letter_or_digit() {
        assert!(matches!(
            ManagedName::parse("-name"),
            Err(ManagedNameError::InvalidGrammar)
        ));
        assert!(matches!(
            ManagedName::parse("name-"),
            Err(ManagedNameError::InvalidGrammar)
        ));
        assert!(matches!(
            ManagedName::parse("/name"),
            Err(ManagedNameError::InvalidGrammar)
        ));
        assert!(ManagedName::parse("a-name").is_ok());
        assert!(ManagedName::parse("name-a").is_ok());
    }

    #[test]
    fn interior_bytes_must_be_safe() {
        assert!(matches!(
            ManagedName::parse("na_m.e"),
            Err(ManagedNameError::InvalidGrammar)
        ));
        assert!(matches!(
            ManagedName::parse("na me"),
            Err(ManagedNameError::InvalidGrammar)
        ));
        assert!(matches!(
            ManagedName::parse("NAME"),
            Err(ManagedNameError::InvalidGrammar)
        ));
        assert!(ManagedName::parse("a-b-c-d").is_ok());
        assert!(ManagedName::parse("camelcase-blah").is_ok());
    }

    #[test]
    fn reserved_names_are_rejected() {
        for reserved in RESERVED_NAMES {
            assert_eq!(
                ManagedName::parse(reserved),
                Err(ManagedNameError::Reserved),
                "{reserved} must treat as reserved"
            );
        }
    }

    #[test]
    fn exact_boundary_lengths_are_accepted() {
        let exactly_max: String = std::iter::repeat('a')
            .take(MAX_MANAGED_NAME_BYTES)
            .collect();
        assert!(ManagedName::parse(&exactly_max).is_ok());
        let one_over_max: String = std::iter::repeat('a')
            .take(MAX_MANAGED_NAME_BYTES + 1)
            .collect();
        assert_eq!(
            ManagedName::parse(&one_over_max),
            Err(ManagedNameError::TooLong)
        );
    }

    #[test]
    fn dot_slash_components_are_rejected() {
        assert!(matches!(
            ManagedName::parse("."),
            Err(ManagedNameError::InvalidGrammar)
        ));
        assert!(matches!(
            ManagedName::parse(".."),
            Err(ManagedNameError::InvalidGrammar)
        ));
        assert!(matches!(
            ManagedName::parse("a../b"),
            Err(ManagedNameError::InvalidGrammar)
        ));
    }
}
