use crate::entities::schemars::{gen::SchemaGenerator, schema::Schema};
use core::fmt;
use http::status::InvalidStatusCode;
use serde::{
    de,
    de::{Unexpected, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};
use std::{convert::TryInto, str::FromStr};

#[derive(Debug, Clone, Hash, PartialEq, Eq, Ord, PartialOrd)]
pub struct StatusCode(pub http::StatusCode);

impl schemars::JsonSchema for StatusCode {
    fn schema_name() -> String {
        unimplemented!()
    }

    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        unimplemented!()
    }
}

impl Serialize for StatusCode {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        http_serde::status_code::serialize(&self.0, serializer)
    }
}

struct StatusVisitor;

impl<'de> Visitor<'de> for StatusVisitor {
    type Value = StatusCode;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "valid status code")
    }

    fn visit_i32<E: de::Error>(self, val: i32) -> Result<Self::Value, E> {
        let v = val
            .try_into()
            .map_err(|_| de::Error::custom("bad status code"))?;
        self.visit_u16(v)
    }

    fn visit_i16<E: de::Error>(self, val: i16) -> Result<Self::Value, E> {
        let v = val
            .try_into()
            .map_err(|_| de::Error::custom("bad status code"))?;
        self.visit_u16(v)
    }

    fn visit_u8<E: de::Error>(self, val: u8) -> Result<Self::Value, E> {
        let v = val
            .try_into()
            .map_err(|_| de::Error::custom("bad status code"))?;
        self.visit_u16(v)
    }

    fn visit_u32<E: de::Error>(self, val: u32) -> Result<Self::Value, E> {
        let v = val
            .try_into()
            .map_err(|_| de::Error::custom("bad status code"))?;
        self.visit_u16(v)
    }

    fn visit_i64<E: de::Error>(self, val: i64) -> Result<Self::Value, E> {
        let v = val
            .try_into()
            .map_err(|_| de::Error::custom("bad status code"))?;
        self.visit_u16(v)
    }

    fn visit_u64<E: de::Error>(self, val: u64) -> Result<Self::Value, E> {
        let v = val
            .try_into()
            .map_err(|_| de::Error::custom("bad status code"))?;
        self.visit_u16(v)
    }

    fn visit_u16<E: de::Error>(self, val: u16) -> Result<Self::Value, E> {
        http::StatusCode::from_u16(val)
            .map_err(|_| de::Error::invalid_value(Unexpected::Unsigned(val.into()), &self))
            .map(StatusCode)
    }
}

impl<'de> Deserialize<'de> for StatusCode {
    fn deserialize<D>(deserializer: D) -> Result<StatusCode, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_u16(StatusVisitor)
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Ord, PartialOrd)]
pub enum StatusCodeRange {
    Single(http::StatusCode),
    Range(http::StatusCode, http::StatusCode),
    List(Vec<http::StatusCode>),
}

impl schemars::JsonSchema for StatusCodeRange {
    fn schema_name() -> String {
        unimplemented!()
    }

    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        unimplemented!()
    }
}

impl ToString for StatusCodeRange {
    fn to_string(&self) -> String {
        match self {
            StatusCodeRange::Single(code) => code.as_u16().to_string(),
            StatusCodeRange::Range(from, to) => {
                format!("{}-{}", from.as_u16(), to.as_u16())
            }
            StatusCodeRange::List(codes) => {
                let v = codes
                    .iter()
                    .map(|c| c.as_u16().to_string())
                    .collect::<Vec<_>>();
                v.join(",")
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StatusCodeRangeParseError {
    #[error("bad wildcard format")]
    BadWildcard,

    #[error("malformed")]
    Malformed,

    #[error("bad status-code")]
    BadStatusCode(InvalidStatusCode),
}

impl StatusCodeRange {
    pub fn is_belongs(&self, code: &http::StatusCode) -> bool {
        match self {
            StatusCodeRange::Single(single_code) => code == single_code,
            StatusCodeRange::Range(from, to) => code >= from && code <= to,
            StatusCodeRange::List(codes) => codes.iter().any(|c| c == code),
        }
    }
}

impl FromStr for StatusCodeRange {
    type Err = StatusCodeRangeParseError;

    fn from_str(val: &str) -> Result<Self, Self::Err> {
        if val.len() == 7 && val.chars().nth(3).unwrap() == '-' {
            let (from, to) = val.split_at(3);
            let from = from.parse().unwrap();
            let to = to[1..].parse().unwrap();
            return Ok(StatusCodeRange::Range(
                http::StatusCode::from_u16(from).unwrap(),
                http::StatusCode::from_u16(to).unwrap(),
            ));
        }
        if val.contains(',') {
            return Ok(StatusCodeRange::List(
                val.split(',')
                    .map(|s| {
                        s.parse()
                            .map_err(|_e| StatusCodeRangeParseError::Malformed)
                            .and_then(|code| {
                                http::StatusCode::from_u16(code)
                                    .map_err(StatusCodeRangeParseError::BadStatusCode)
                            })
                    })
                    .collect::<Result<Vec<http::StatusCode>, _>>()?,
            ));
        }
        if val == "*" {
            return Ok(StatusCodeRange::Range(
                http::StatusCode::from_u16(100).unwrap(),
                http::StatusCode::from_u16(599).unwrap(),
            ));
        }
        if val.len() != 3 {
            return Err(StatusCodeRangeParseError::Malformed);
        }
        if val.chars().nth(1).unwrap() == 'x' && val.chars().nth(2).unwrap() == 'x' {
            let first_char = val.chars().next().unwrap();
            if first_char == 'x' {
                return Ok(StatusCodeRange::Range(
                    http::StatusCode::from_u16(100).unwrap(),
                    http::StatusCode::from_u16(599).unwrap(),
                ));
            }
            if !('0'..='9').contains(&first_char) {
                return Err(StatusCodeRangeParseError::BadWildcard);
            };
            let from = format!("{}00", first_char).parse().unwrap();
            let to = format!("{}99", first_char).parse().unwrap();
            return Ok(StatusCodeRange::Range(
                http::StatusCode::from_u16(from).unwrap(),
                http::StatusCode::from_u16(to).unwrap(),
            ));
        }
        if let Ok(code) = val.parse() {
            return Ok(StatusCodeRange::Single(
                http::StatusCode::from_u16(code).unwrap(),
            ));
        }

        Err(StatusCodeRangeParseError::Malformed)
    }
}

impl Serialize for StatusCodeRange {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_str())
    }
}

struct StatusCodeRangeVisitor;

impl<'de> Visitor<'de> for StatusCodeRangeVisitor {
    type Value = StatusCodeRange;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "status-codes range")
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

impl<'de> Deserialize<'de> for StatusCodeRange {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(StatusCodeRangeVisitor)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::config_core::CatchMatcher;

    #[test]
    pub fn test_parsing() {
        const YAML: &str = r#"---
- "status-code:200,201,203"
- "status-code:5xx"
- "status-code:500-550"
- "status-code:500"
- "status-code:xxx"
- "status-code:*"
"#;
        let parsed = serde_yaml::from_str::<Vec<CatchMatcher>>(YAML).unwrap();
        assert_eq!(
            parsed[0],
            CatchMatcher::StatusCode(StatusCodeRange::List(vec![
                http::StatusCode::from_u16(200).unwrap(),
                http::StatusCode::from_u16(201).unwrap(),
                http::StatusCode::from_u16(203).unwrap(),
            ]))
        );
        assert_eq!(
            parsed[1],
            CatchMatcher::StatusCode(StatusCodeRange::Range(
                http::StatusCode::from_u16(500).unwrap(),
                http::StatusCode::from_u16(599).unwrap(),
            ))
        );
        assert_eq!(
            parsed[2],
            CatchMatcher::StatusCode(StatusCodeRange::Range(
                http::StatusCode::from_u16(500).unwrap(),
                http::StatusCode::from_u16(550).unwrap(),
            ))
        );
        assert_eq!(
            parsed[3],
            CatchMatcher::StatusCode(StatusCodeRange::Single(
                http::StatusCode::from_u16(500).unwrap()
            ))
        );
        assert_eq!(
            parsed[4],
            CatchMatcher::StatusCode(StatusCodeRange::Range(
                http::StatusCode::from_u16(100).unwrap(),
                http::StatusCode::from_u16(599).unwrap(),
            ))
        );
        assert_eq!(
            parsed[5],
            CatchMatcher::StatusCode(StatusCodeRange::Range(
                http::StatusCode::from_u16(100).unwrap(),
                http::StatusCode::from_u16(599).unwrap(),
            ))
        );
    }
}
