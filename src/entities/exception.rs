use crate::entities::{ExceptionSegment, StringIdentifierParseError};
use schemars::{
    gen::SchemaGenerator,
    schema::{InstanceType, Schema, SchemaObject, StringValidation},
    JsonSchema,
};
use serde::{de, de::Visitor, Deserialize, Deserializer, Serialize, Serializer};
use std::{convert::TryFrom, fmt, str::FromStr};

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct Exception(pub Vec<ExceptionSegment>);

impl Exception {
    pub fn from_segments(segments: &[ExceptionSegment]) -> Self {
        Exception(segments.to_vec())
    }
}

impl JsonSchema for Exception {
    fn schema_name() -> String {
        "Exception".to_string()
    }

    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        SchemaObject {
            instance_type: Some(InstanceType::String.into()),
            string: Some(Box::new(StringValidation {
                min_length: Some(1),
                ..Default::default()
            })),
            ..Default::default()
        }
        .into()
    }
}

impl fmt::Display for Exception {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let len = self.0.len();
        for (idx, segment) in self.0.iter().enumerate() {
            f.write_str(segment.to_string().as_str())?;
            if idx != len - 1 {
                write!(f, ":")?;
            }
        }

        Ok(())
    }
}

impl<'a> TryFrom<&'a str> for Exception {
    type Error = ExceptionParseError;

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl TryFrom<String> for Exception {
    type Error = ExceptionParseError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

struct ExceptionVisitor;

impl<'de> Visitor<'de> for ExceptionVisitor {
    type Value = Exception;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "string separated with ':'")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value
            .parse()
            .map_err(|e| de::Error::custom(format!("bad format: {} on {}", e, value)))
    }
}

impl Serialize for Exception {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_str())
    }
}

impl<'de> Deserialize<'de> for Exception {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(ExceptionVisitor)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ExceptionParseError {
    #[error("exception segment error: {_0}")]
    SegmentError(#[from] StringIdentifierParseError),
}

impl FromStr for Exception {
    type Err = ExceptionParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let segments = s
            .split(':')
            .map(|seg| seg.parse().map_err(Self::Err::from))
            .collect::<Result<Vec<_>, Self::Err>>()?;
        Ok(Exception(segments))
    }
}
