use crate::{
    config_core::{
        referenced::Container,
        status_code::{StatusCode, StatusCodeRangeParseError},
        StaticResponse, StatusCodeRange,
    },
    entities::{
        schemars::{gen::SchemaGenerator, schema::Schema},
        Exception, ExceptionParseError, StaticResponseName,
    },
};
use core::fmt;
use schemars::{
    schema::{InstanceType, Metadata, SchemaObject},
    JsonSchema,
};
use serde::{de, de::Visitor, Deserialize, Deserializer, Serialize, Serializer};
use smol_str::SmolStr;
use std::{collections::BTreeMap, str::FromStr};

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq, JsonSchema)]
// #[schemars(deny_unknown_fields)]
#[serde(tag = "action")]
pub enum CatchAction {
    #[serde(rename = "respond")]
    StaticResponse {
        #[serde(rename = "static-response")]
        static_response: Container<StaticResponse, StaticResponseName>,

        #[serde(rename = "status-code", default)]
        status_code: Option<StatusCode>,

        #[serde(default)]
        data: BTreeMap<SmolStr, SmolStr>,
    },

    #[serde(rename = "throw")]
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

impl JsonSchema for CatchMatcher {
    fn schema_name() -> String {
        "CatchMatcher".to_string()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        SchemaObject {
            metadata: Some(Box::new(Metadata {
                title: Some("Matcher for exception catching".to_string()),
                description: Some(
                    "string starting with 'status-code:' or 'exception:'".to_string(),
                ),
                ..Default::default()
            })),
            instance_type: Some(InstanceType::String.into()),
            ..Default::default()
        }
        .into()
    }
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

impl<'de> Deserialize<'de> for CatchMatcher {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(CatchMatcherVisitor)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq, JsonSchema)]
// #[schemars(deny_unknown_fields)]
pub struct RescueItem {
    pub catch: CatchMatcher,

    #[serde(flatten)]
    pub handle: CatchAction,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn test_schema() {
        serde_json::to_string_pretty(&schemars::schema_for!(CatchMatcher)).unwrap();
    }

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
action: throw
exception: proxy:error
"#
            )
            .unwrap()
        );
    }
}
