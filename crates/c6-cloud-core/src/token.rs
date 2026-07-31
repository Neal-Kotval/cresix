use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::{fmt, str::FromStr};
use thiserror::Error;

/// Disjoint authentication surfaces. A recognized prefix classifies a token;
/// it never authenticates it without verifier lookup and constant-time proof
/// comparison by the receiving service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenClass {
    Bootstrap,
    CloudSession,
    Csrf,
    Connector,
}

impl TokenClass {
    pub const ALL: [Self; 4] = [
        Self::Bootstrap,
        Self::CloudSession,
        Self::Csrf,
        Self::Connector,
    ];

    pub const fn prefix(self) -> &'static str {
        match self {
            Self::Bootstrap => "c6b_v1",
            Self::CloudSession => "c6s_v1",
            Self::Csrf => "c6f_v1",
            Self::Connector => "c6x_v1",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TokenParseError {
    #[error("unknown credential class")]
    UnknownClass,
    #[error("malformed credential")]
    Malformed,
}

impl FromStr for TokenClass {
    type Err = TokenParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|class| class.prefix() == value)
            .ok_or(TokenParseError::UnknownClass)
    }
}

/// Plaintext token that is safe to pass through wire DTOs but redacts itself
/// from `Debug` and `Display`. Callers must avoid logging serialized requests.
#[derive(Clone, PartialEq, Eq)]
pub struct SecretToken(String);

impl SecretToken {
    pub const MAX_LEN: usize = 256;

    pub fn parse(value: impl Into<String>) -> Result<Self, TokenParseError> {
        let value = value.into();
        ParsedToken::parse(&value)?;
        Ok(Self(value))
    }

    pub fn expose_secret(&self) -> &str {
        &self.0
    }

    pub fn parsed(&self) -> ParsedToken<'_> {
        // Construction validates the invariant.
        ParsedToken::parse(&self.0).expect("SecretToken invariant")
    }
}

impl fmt::Debug for SecretToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretToken([REDACTED])")
    }
}
impl fmt::Display for SecretToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}
impl FromStr for SecretToken {
    type Err = TokenParseError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}
impl Serialize for SecretToken {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}
impl<'de> Deserialize<'de> for SecretToken {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::parse(String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ParsedToken<'a> {
    pub class: TokenClass,
    pub public_id: &'a str,
    secret: &'a str,
}

impl<'a> ParsedToken<'a> {
    pub fn parse(value: &'a str) -> Result<ParsedToken<'a>, TokenParseError> {
        if value.len() > SecretToken::MAX_LEN {
            return Err(TokenParseError::Malformed);
        }
        let (class, remainder) = TokenClass::ALL
            .into_iter()
            .find_map(|class| value.strip_prefix(class.prefix()).map(|rest| (class, rest)))
            .ok_or(TokenParseError::UnknownClass)?;
        let remainder = remainder
            .strip_prefix('_')
            .ok_or(TokenParseError::Malformed)?;
        // The public identifier is fixed-width because `_` is itself valid
        // base64url. Searching for a separator would otherwise parse some
        // legitimately generated identifiers at the wrong boundary.
        if remainder.as_bytes().get(16) != Some(&b'_') {
            return Err(TokenParseError::Malformed);
        }
        let public_id = &remainder[..16];
        let secret = &remainder[17..];
        if !(32..=128).contains(&secret.len()) || !is_base64url(public_id) || !is_base64url(secret)
        {
            return Err(TokenParseError::Malformed);
        }
        Ok(ParsedToken {
            class,
            public_id,
            secret,
        })
    }

    /// Authentication code needs the proof to derive/compare a verifier.
    /// This value must never be logged or persisted as plaintext.
    pub fn expose_proof(self) -> &'a str {
        self.secret
    }
}

impl fmt::Debug for ParsedToken<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ParsedToken")
            .field("class", &self.class)
            .field("public_id", &self.public_id)
            .field("proof", &"[REDACTED]")
            .finish()
    }
}

fn is_base64url(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "c6x_v1_AAAAAAAAAAAAAAAA_BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB";

    #[test]
    fn parses_and_classifies_without_exposing_proof() {
        let token = SecretToken::parse(TOKEN).unwrap();
        let parsed = token.parsed();
        assert_eq!(parsed.class, TokenClass::Connector);
        assert_eq!(parsed.public_id, "AAAAAAAAAAAAAAAA");
        assert_eq!(parsed.expose_proof(), "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB");
        assert!(!format!("{token:?}").contains("BBBB"));
        assert_eq!(token.to_string(), "[REDACTED]");
    }

    #[test]
    fn rejects_cross_class_and_malformed_tokens() {
        for token in [
            "bearer_AAAAAAAAAAAAAAAA_BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
            "c6x_v1_short_BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
            "c6x_v1_AAAAAAAAAAAAAAAA_short",
            "c6x_v1_AAAAAAAAAAAAAAAA_BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB!",
        ] {
            assert!(SecretToken::parse(token).is_err(), "accepted {token}");
        }
    }

    #[test]
    fn serde_redacts_debug_but_preserves_wire_value() {
        let token = SecretToken::parse(TOKEN).unwrap();
        let json = serde_json::to_string(&token).unwrap();
        assert_eq!(serde_json::from_str::<SecretToken>(&json).unwrap(), token);
    }
}
