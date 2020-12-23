use crate::config_core::status_code::{StatusCode, StatusCodeRangeParseError};
use crate::config_core::StatusCodeRange;
use crate::entities::{ExceptionSegment, StaticResponseName, StringIdentifierParseError};
use core::fmt;
use serde::de::Visitor;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use smol_str::SmolStr;
use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::str::FromStr;

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
#[serde(tag = "action", deny_unknown_fields)]
pub enum CatchAction {
    #[serde(rename = "respond")]
    StaticResponse {
        #[serde(rename = "static-response")]
        name: StaticResponseName,

        #[serde(rename = "status-code", default)]
        status_code: Option<StatusCode>,

        #[serde(default)]
        data: BTreeMap<SmolStr, SmolStr>,
    },

    #[serde(rename = "throw-exception")]
    Throw {
        #[serde(rename = "exception")]
        exception: Exception,

        #[serde(default)]
        data: BTreeMap<SmolStr, SmolStr>,
    },

    #[serde(rename = "next-handler")]
    NextHandler,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum CatchMatcher {
    StatusCode(StatusCodeRange),
    Exception(Exception),
}

impl ToString for CatchMatcher {
    fn to_string(&self) -> String {
        match self {
            CatchMatcher::StatusCode(codes) => {
                format!("status-code:{}", codes.to_string())
            }
            CatchMatcher::Exception(exception) => {
                format!("exception:{}", exception.to_string())
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CatchMatcherParseError {
    #[error("unknown kind")]
    UnknownKind,

    #[error("exception matcher parse error: {_0}")]
    ExceptionParseError(#[from] ExceptionParseError),

    #[error("status code range parse error: {_0}")]
    StatusCodeRangeParseError(#[from] StatusCodeRangeParseError),
}

impl FromStr for CatchMatcher {
    type Err = CatchMatcherParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        const STATUS_CODE_KIND: &str = "status-code:";
        const EXCEPTION_KIND: &str = "exception:";

        if s.starts_with(STATUS_CODE_KIND) {
            let status_codes = s.strip_prefix(STATUS_CODE_KIND).unwrap().parse()?;
            Ok(CatchMatcher::StatusCode(status_codes))
        } else if s.starts_with(EXCEPTION_KIND) {
            let exception = s.strip_prefix(EXCEPTION_KIND).unwrap().parse()?;
            Ok(CatchMatcher::Exception(exception))
        } else {
            Err(CatchMatcherParseError::UnknownKind)
        }
    }
}

impl Serialize for CatchMatcher {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_str())
    }
}

struct CatchMatcherVisitor;

impl<'de> Visitor<'de> for CatchMatcherVisitor {
    type Value = CatchMatcher;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "string separated with ':' starting with `status-codes` or `exception`"
        )
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

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct Exception(pub Vec<ExceptionSegment>);

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

impl<'de> Deserialize<'de> for CatchMatcher {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(CatchMatcherVisitor)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
pub struct RescueItem {
    pub catch: CatchMatcher,

    #[serde(flatten)]
    pub handle: CatchAction,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn test_deserialize() {
        assert_eq!(
            RescueItem {
                catch: CatchMatcher::StatusCode(StatusCodeRange::Range(
                    http::StatusCode::from_u16(500).unwrap(),
                    http::StatusCode::from_u16(599).unwrap(),
                )),
                handle: CatchAction::NextHandler
            },
            serde_yaml::from_str::<RescueItem>(
                r#"
---
catch: status-code:500-599
action: next-handler
"#
            )
            .unwrap()
        );

        assert_eq!(
            RescueItem {
                catch: CatchMatcher::Exception(Exception(vec![
                    "proxy".parse().unwrap(),
                    "timeout".parse().unwrap()
                ])),
                handle: CatchAction::Throw {
                    exception: "proxy:error".parse().unwrap(),
                    data: Default::default()
                }
            },
            serde_yaml::from_str::<RescueItem>(
                r#"
---
catch: exception:proxy:timeout
action: throw-exception
exception: proxy:error
"#
            )
            .unwrap()
        );
    }
}
