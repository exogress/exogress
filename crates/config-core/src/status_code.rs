use core::fmt;
use serde::de::{Unexpected, Visitor};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::convert::TryInto;

#[derive(Debug, Clone, Hash, PartialEq, Eq, Ord, PartialOrd)]
pub struct StatusCode(pub http::StatusCode);

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

impl Serialize for StatusCodeRange {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        match self {
            StatusCodeRange::Single(code) => http_serde::status_code::serialize(code, serializer),
            StatusCodeRange::Range(from, to) => {
                let s = format!("{}-{}", from, to);
                serializer.serialize_str(&s)
            }
            StatusCodeRange::List(codes) => {
                let v = codes.iter().map(|c| c.to_string()).collect::<Vec<_>>();
                let s = v.join(",");
                serializer.serialize_str(&s)
            }
        }
    }
}

struct StatusCodeRangeVisitor;

impl<'de> Visitor<'de> for StatusCodeRangeVisitor {
    type Value = StatusCodeRange;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "valid status code")
    }

    fn visit_i16<E: de::Error>(self, val: i16) -> Result<Self::Value, E> {
        let v = val
            .try_into()
            .map_err(|_| de::Error::custom("bad status code"))?;
        self.visit_u16(v)
    }

    fn visit_i32<E: de::Error>(self, val: i32) -> Result<Self::Value, E> {
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

    fn visit_u8<E: de::Error>(self, val: u8) -> Result<Self::Value, E> {
        let v = val
            .try_into()
            .map_err(|_| de::Error::custom("bad status code"))?;
        self.visit_u16(v)
    }

    fn visit_u16<E: de::Error>(self, val: u16) -> Result<Self::Value, E> {
        http::StatusCode::from_u16(val)
            .map_err(|_| de::Error::invalid_value(Unexpected::Unsigned(val.into()), &self))
            .map(StatusCodeRange::Single)
    }

    fn visit_u32<E: de::Error>(self, val: u32) -> Result<Self::Value, E> {
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

    fn visit_str<E: de::Error>(self, val: &str) -> Result<Self::Value, E> {
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
                            .map_err(|e| de::Error::custom(&format!("bad status code: {}", e,)))
                            .and_then(|code| {
                                http::StatusCode::from_u16(code)
                                    .map_err(|_| de::Error::custom("bad status code"))
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
            return Err(de::Error::custom("bad status code"));
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
                return Err(de::Error::custom("bad wildcard format"));
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

        Err(de::Error::custom("malformed"))
    }
}

impl<'de> Deserialize<'de> for StatusCodeRange {
    fn deserialize<D>(deserializer: D) -> Result<StatusCodeRange, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(StatusCodeRangeVisitor)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn test_parsing() {
        const YAML: &str = r#"---
- "200,201,203"
- 500
- "5xx"
- "500-550"
- "500"
- xxx
- "*"
"#;
        let parsed = serde_yaml::from_str::<Vec<StatusCodeRange>>(YAML).unwrap();
        assert_eq!(
            parsed[0],
            StatusCodeRange::List(vec![
                http::StatusCode::from_u16(200).unwrap(),
                http::StatusCode::from_u16(201).unwrap(),
                http::StatusCode::from_u16(203).unwrap(),
            ])
        );
        assert_eq!(
            parsed[1],
            StatusCodeRange::Single(http::StatusCode::from_u16(500).unwrap())
        );
        assert_eq!(
            parsed[2],
            StatusCodeRange::Range(
                http::StatusCode::from_u16(500).unwrap(),
                http::StatusCode::from_u16(599).unwrap(),
            )
        );
        assert_eq!(
            parsed[3],
            StatusCodeRange::Range(
                http::StatusCode::from_u16(500).unwrap(),
                http::StatusCode::from_u16(550).unwrap(),
            )
        );
        assert_eq!(
            parsed[4],
            StatusCodeRange::Single(http::StatusCode::from_u16(500).unwrap())
        );
        assert_eq!(
            parsed[5],
            StatusCodeRange::Range(
                http::StatusCode::from_u16(100).unwrap(),
                http::StatusCode::from_u16(599).unwrap(),
            )
        );
        assert_eq!(
            parsed[6],
            StatusCodeRange::Range(
                http::StatusCode::from_u16(100).unwrap(),
                http::StatusCode::from_u16(599).unwrap(),
            )
        );
    }
}
