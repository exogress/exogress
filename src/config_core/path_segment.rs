use percent_encoding::percent_decode_str;
use serde::de::Visitor;

use crate::config_core::path::ANY_SEGMENTS_MATCH_STR;
use schemars::JsonSchema;
use serde::{de, Deserialize, Deserializer, Serialize};
use smol_str::SmolStr;
use std::{
    fmt,
    str::{FromStr, Utf8Error},
};

#[derive(thiserror::Error, Debug)]
pub enum PathSegmentParseError {
    #[error("empty segment disallowed")]
    Empty,

    #[error("special path segments (`.`, `..`, `/`, `*`) disallowed")]
    Special,

    #[error("path segment is not properly percent-encoded: `{0}`")]
    Encoding(Utf8Error),

    #[error("zero byte in path disallowed")]
    ZeroByte,
}

#[derive(Debug, Hash, Eq, Serialize, PartialEq, Clone, JsonSchema)]
#[serde(transparent)]
pub struct UrlPathSegment(SmolStr);

impl AsRef<str> for UrlPathSegment {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl AsRef<[u8]> for UrlPathSegment {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl FromStr for UrlPathSegment {
    type Err = PathSegmentParseError;

    fn from_str(segment: &str) -> Result<Self, Self::Err> {
        if segment.is_empty() {
            return Err(PathSegmentParseError::Empty);
        }

        if segment == "." || segment == ".." || segment == ANY_SEGMENTS_MATCH_STR {
            return Err(PathSegmentParseError::Special);
        }

        if segment.contains('/') {
            return Err(PathSegmentParseError::Special);
        }

        match percent_decode_str(segment).decode_utf8() {
            Err(e) => {
                return Err(PathSegmentParseError::Encoding(e));
            }
            Ok(decoded) => {
                if decoded.contains('\0') {
                    return Err(PathSegmentParseError::ZeroByte);
                }
            }
        }

        Ok(UrlPathSegment(segment.into()))
    }
}

impl UrlPathSegment {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

pub(crate) struct UrlPathSegmentVisitor;

impl<'de> Visitor<'de> for UrlPathSegmentVisitor {
    type Value = UrlPathSegment;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "Path segment")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        match UrlPathSegment::from_str(value) {
            Ok(segment) => Ok(segment),
            Err(e) => Err(de::Error::custom(e)),
        }
    }
}

impl<'de> Deserialize<'de> for UrlPathSegment {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(UrlPathSegmentVisitor)
    }
}
