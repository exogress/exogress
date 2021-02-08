use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

use crate::config_core::UrlPathSegment;
use serde::de::Visitor;
use std::fmt;

pub type RefNumber = u8;

#[derive(Debug, Hash, Clone, Eq, PartialEq)]
pub enum PathSegmentsModify {
    Reference(RefNumber),
    Single(UrlPathSegment),
}

impl ToString for PathSegmentsModify {
    fn to_string(&self) -> String {
        match self {
            PathSegmentsModify::Reference(r) => format!("${}", r),
            PathSegmentsModify::Single(s) => AsRef::<str>::as_ref(s).to_string(),
        }
    }
}

impl Serialize for PathSegmentsModify {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

impl From<UrlPathSegment> for PathSegmentsModify {
    fn from(part: UrlPathSegment) -> Self {
        PathSegmentsModify::Single(part)
    }
}

pub struct PathSegmentRewriteVisitor;

impl<'de> Visitor<'de> for PathSegmentRewriteVisitor {
    type Value = PathSegmentsModify;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "single path segment or \"$NUM\" or \"$NUM:GROUP\""
        )
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if value.starts_with("$") {
            let num: RefNumber = value[1..]
                .parse()
                .map_err(|e| de::Error::custom(format!("bad reference number: {}", e)))?;
            Ok(PathSegmentsModify::Reference(num))
        } else {
            match value.parse() {
                Ok(r) => Ok(PathSegmentsModify::Single(r)),
                Err(e) => Err(de::Error::custom(e)),
            }
        }
    }
}

impl<'de> Deserialize<'de> for PathSegmentsModify {
    fn deserialize<D>(deserializer: D) -> Result<PathSegmentsModify, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(PathSegmentRewriteVisitor)
    }
}
