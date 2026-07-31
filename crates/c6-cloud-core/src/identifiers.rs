use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::{fmt, str::FromStr};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IdentifierError {
    #[error("{kind} must be between {min} and {max} characters")]
    Length {
        kind: &'static str,
        min: usize,
        max: usize,
    },
    #[error("{kind} must start and end with a lowercase ASCII letter or digit")]
    Boundary { kind: &'static str },
    #[error("{kind} may contain only lowercase ASCII letters, digits, and single hyphens")]
    Characters { kind: &'static str },
    #[error("{kind} is reserved")]
    Reserved { kind: &'static str },
    #[error("installation label must not have leading/trailing whitespace or control characters")]
    UnsafeLabel,
}

fn validate_slug(
    value: &str,
    kind: &'static str,
    min: usize,
    max: usize,
    reserved: &[&str],
) -> Result<(), IdentifierError> {
    let len = value.len();
    if !(min..=max).contains(&len) {
        return Err(IdentifierError::Length { kind, min, max });
    }
    let bytes = value.as_bytes();
    let is_alnum = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();
    if !is_alnum(bytes[0]) || !is_alnum(bytes[len - 1]) {
        return Err(IdentifierError::Boundary { kind });
    }
    if bytes.iter().any(|byte| !is_alnum(*byte) && *byte != b'-')
        || bytes.windows(2).any(|pair| pair == b"--")
    {
        return Err(IdentifierError::Characters { kind });
    }
    if reserved.contains(&value) {
        return Err(IdentifierError::Reserved { kind });
    }
    Ok(())
}

macro_rules! slug_identifier {
    ($name:ident, $kind:literal, $min:literal, $max:literal, $reserved:expr) => {
        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, IdentifierError> {
                let value = value.into();
                validate_slug(&value, $kind, $min, $max, $reserved)?;
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub fn into_inner(self) -> String {
                self.0
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_tuple(stringify!($name))
                    .field(&self.0)
                    .finish()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl FromStr for $name {
            type Err = IdentifierError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::new(value)
            }
        }

        impl TryFrom<String> for $name {
            type Error = IdentifierError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::new(value).map_err(de::Error::custom)
            }
        }
    };
}

const RESERVED_NAMES: &[&str] = &[
    "api",
    "assets",
    "bootstrap",
    "connector",
    "directory",
    "installations",
    "login",
    "logout",
    "relay",
    "session",
    "settings",
    "status",
    "workspaces",
];

slug_identifier!(AccountHandle, "account handle", 3, 39, RESERVED_NAMES);
slug_identifier!(
    WorkspaceNamespace,
    "workspace namespace",
    3,
    63,
    RESERVED_NAMES
);
slug_identifier!(ProjectSlug, "project slug", 1, 100, &[]);

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InstallationLabel(String);

impl InstallationLabel {
    pub const MAX_LEN: usize = 80;

    pub fn new(value: impl Into<String>) -> Result<Self, IdentifierError> {
        let value = value.into();
        if value.is_empty() || value.len() > Self::MAX_LEN {
            return Err(IdentifierError::Length {
                kind: "installation label",
                min: 1,
                max: Self::MAX_LEN,
            });
        }
        if value.trim() != value || value.chars().any(char::is_control) {
            return Err(IdentifierError::UnsafeLabel);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Debug for InstallationLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("InstallationLabel").field(&self.0).finish()
    }
}
impl fmt::Display for InstallationLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl FromStr for InstallationLabel {
    type Err = IdentifierError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}
impl Serialize for InstallationLabel {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}
impl<'de> Deserialize<'de> for InstallationLabel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_accept_canonical_values() {
        assert_eq!(AccountHandle::new("neal-6").unwrap().as_str(), "neal-6");
        assert_eq!(
            WorkspaceNamespace::new("paper-street").unwrap().as_str(),
            "paper-street"
        );
        assert_eq!(ProjectSlug::new("x").unwrap().as_str(), "x");
    }

    #[test]
    fn slugs_reject_ambiguous_or_unsafe_values() {
        for value in ["Neal", "-neal", "neal-", "neal--six", "ne_al", "néal"] {
            assert!(AccountHandle::new(value).is_err(), "accepted {value}");
        }
        assert!(WorkspaceNamespace::new("api").is_err());
        assert!(ProjectSlug::new("a/b").is_err());
    }

    #[test]
    fn serde_cannot_bypass_validation() {
        assert!(serde_json::from_str::<WorkspaceNamespace>(r#""UPPER""#).is_err());
        let namespace = WorkspaceNamespace::new("paper-street").unwrap();
        assert_eq!(
            serde_json::to_string(&namespace).unwrap(),
            r#""paper-street""#
        );
    }

    #[test]
    fn labels_reject_log_forging_and_padding() {
        assert!(InstallationLabel::new(" laptop ").is_err());
        assert!(InstallationLabel::new("laptop\nrevoked").is_err());
        assert!(InstallationLabel::new("work laptop").is_ok());
    }
}
