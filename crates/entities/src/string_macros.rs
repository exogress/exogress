pub const MIN_STRING_IDENTIFIER_LENGTH: usize = 2;
pub const MAX_STRING_IDENTIFIER_LENGTH: usize = 46;

#[derive(thiserror::Error, Debug)]
pub enum StringIdentifierError {
    #[error("exceeded identifier length (should be not more than {MAX_STRING_IDENTIFIER_LENGTH})")]
    ExceededIdentifierLength,

    #[error(
        "insufficient identifier length (should be not less than {MIN_STRING_IDENTIFIER_LENGTH})"
    )]
    InsufficientIdentifierLength,

    #[error("bad character `{0}`. Should be lowercase alphanumeric, _ or -")]
    BadCharacter(std::primitive::char),

    #[error("bad starting character `{0}`. Should be lowercase alphanumeric")]
    BadStartingCharacter(std::primitive::char),
}

#[derive(thiserror::Error, Debug)]
pub enum StringIdentifierParseError {
    #[error("UTF-8 error")]
    Utf8Error(#[from] std::str::Utf8Error),

    #[error("string identifier error: `{0}`")]
    StringIdentifierError(#[from] StringIdentifierError),
}

#[macro_export]
macro_rules! string_type {
    ($x:ident) => {
        paste::item! {
            pub fn [<validate_ $x:snake>](s: &str) -> Result<(), $crate::StringIdentifierError> {
                let len = s.len();

                if len < $crate::MIN_STRING_IDENTIFIER_LENGTH  {
                    return Err($crate::StringIdentifierError::InsufficientIdentifierLength);
                } else if len > $crate::MAX_STRING_IDENTIFIER_LENGTH  {
                    return Err($crate::StringIdentifierError::ExceededIdentifierLength);
                }

                let first_char = *s.chars().peekable().peek().unwrap();

                if !first_char.is_alphanumeric() || !first_char.is_lowercase() {
                    return Err($crate::StringIdentifierError::BadStartingCharacter(first_char));
                }

                for c in s.chars() {
                    if !c.is_alphanumeric() &&
                        !c.is_lowercase() &&
                        c != '-' &&
                        c != '_'
                    {
                        return Err($crate::StringIdentifierError::BadCharacter(c));
                    }
                }

                Ok(())
            }
        }

        #[derive(Debug, Clone, $crate::serde::Serialize, Hash, Eq, PartialEq, Ord, PartialOrd)]
        #[serde(transparent)]
        pub struct $x {
            inner: $crate::String,
        }

        paste::item! {
            struct [<$x Visitor>];
        }

        paste::item! {
            impl<'de> $crate::serde::Deserialize<'de> for $x {
                fn deserialize<D>(deserializer: D) -> Result<$x, D::Error>
                where
                    D: $crate::serde::Deserializer<'de>,
                {
                    deserializer.deserialize_str([<$x Visitor>])
                }
            }
        }

        paste::item! {
            impl<'de> $crate::serde::de::Visitor<'de> for [<$x Visitor>] {
                type Value = $x;

                fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                    formatter.write_fmt(
                        format_args!(
                            "a string between {} and {} alphanumeric characters, _ or -",
                            $crate::MIN_STRING_IDENTIFIER_LENGTH,
                            $crate::MAX_STRING_IDENTIFIER_LENGTH
                        )
                    )
                }

                fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
                where
                    E: $crate::serde::de::Error,
                {
                    use std::str::FromStr;

                    match $x::from_str(value) {
                        Ok(r) => {
                            Ok(r)
                        }
                        Err(e) => {
                            Err($crate::serde::de::Error::custom(e.to_string()))
                        }
                    }
                }
            }
        }

        impl std::fmt::Display for $x {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.inner)
            }
        }

        paste::item! {
            impl std::str::FromStr for $x {
                type Err = $crate::StringIdentifierParseError;

                fn from_str(s: &str) -> Result<Self, Self::Err> {
                    [<validate_ $x:snake>](s)?;
                    Ok($x { inner: s.into() })
                }
            }
        }

        impl std::ops::Deref for $x {
            type Target = str;

            fn deref(&self) -> &Self::Target {
                self.inner.as_str()
            }
        }

        impl $x {
            pub fn as_str(&self) -> &str {
                &self
            }
        }

        impl std::convert::TryFrom<$crate::bytes::Bytes> for $x {
            type Error = $crate::StringIdentifierParseError;

            fn try_from(value: $crate::bytes::Bytes) -> Result<Self, Self::Error> {
                std::str::from_utf8(&value)?.parse()
            }
        }

        impl From<$x> for $crate::bytes::Bytes {
            fn from(v: $x) -> Self {
                $crate::bytes::Bytes::copy_from_slice(v.inner.as_ref())
            }
        }

        impl From<$x> for $crate::String {
            fn from(v: $x) -> Self {
                v.inner.clone()
            }
        }
    };
}

#[cfg(test)]
mod test {
    use bytes::Bytes;
    use std::convert::TryFrom;
    use std::str::FromStr;

    string_type!(TestIdentifier);

    #[test]
    pub fn test_validation() {
        validate_test_identifier("asd!").err().unwrap();
        validate_test_identifier("a").err().unwrap();
        validate_test_identifier("asdf").unwrap();
        validate_test_identifier("asdf-gh").unwrap();
        validate_test_identifier("asdf-gh_asf").unwrap();
        validate_test_identifier("asdf-gh_asfafasgasdgasdgasdgasdfasdfgjasbdgklasbfglkjasbhdlgkjbasdlkjfghaslkdjfhlaskjdhfklj").err().unwrap();
    }

    #[test]
    pub fn parse_string() {
        TestIdentifier::from_str("a").err().unwrap();
        TestIdentifier::from_str("asdf-gh_asf").unwrap();
    }

    #[test]
    pub fn parse_bytes() {
        TestIdentifier::try_from(Bytes::from_static("a".as_bytes()))
            .err()
            .unwrap();
        TestIdentifier::try_from(Bytes::from_static("asd-asdasd-aa-1123".as_bytes())).unwrap();
    }

    #[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
    struct Container {
        id: TestIdentifier,
    }

    #[test]
    pub fn parse_serde() {
        serde_json::from_str::<Container>(r#"{"id": "a"}"#)
            .err()
            .unwrap();
        serde_json::from_str::<Container>(r#"{"id": "asd-asd-123"}"#).unwrap();
    }
}
