use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

use crate::config_core::UrlPathSegmentOrQueryPart;
use serde::de::Visitor;
use std::fmt;

// pub type RefNumber = u8;

#[derive(Debug, Hash, Clone, Eq, PartialEq)]
pub enum PathSegmentRewrite {
    // Reference(RefNumber),
    Single(UrlPathSegmentOrQueryPart),
}

impl AsRef<str> for PathSegmentRewrite {
    fn as_ref(&self) -> &str {
        let Self::Single(s) = self;
        s.as_ref()
    }
}

impl Serialize for PathSegmentRewrite {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        match self {
            // PathSegmentRewrite::Reference(num) => {
            //     serializer.serialize_str(format!("{}{}", REF_STR, num).as_str())
            // }
            PathSegmentRewrite::Single(s) => serializer.serialize_str(s.as_str()),
        }
    }
}

impl From<UrlPathSegmentOrQueryPart> for PathSegmentRewrite {
    fn from(part: UrlPathSegmentOrQueryPart) -> Self {
        PathSegmentRewrite::Single(part)
    }
}

pub struct PathSegmentRewriteVisitor;

impl<'de> Visitor<'de> for PathSegmentRewriteVisitor {
    type Value = PathSegmentRewrite;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "single path segment \"?\" or \"*\"")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        // if value.starts_with(REF_STR) {
        //     let num: RefNumber = value[REF_STR.len()..]
        //         .parse()
        //         .map_err(|e| de::Error::custom(format!("bad reference number: {}", e)))?;
        //     Ok(PathSegmentRewrite::Reference(num))
        // } else {
        match value.parse() {
            Ok(r) => Ok(PathSegmentRewrite::Single(r)),
            Err(e) => Err(de::Error::custom(e)),
        }
        // }
    }
}

impl<'de> Deserialize<'de> for PathSegmentRewrite {
    fn deserialize<D>(deserializer: D) -> Result<PathSegmentRewrite, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(PathSegmentRewriteVisitor)
    }
}
