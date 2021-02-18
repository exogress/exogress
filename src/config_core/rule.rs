use crate::{
    config_core::{
        catch::RescueItem, is_default, methods::MethodMatcher, path::MatchingPath,
        path_modify::PathSegmentsModify, query::QueryMatcher, referenced::Container,
        StaticResponse, StatusCode, StatusCodeRange,
    },
    entities::{
        schemars::{gen::SchemaGenerator, schema::Schema},
        ProfileName, StaticResponseName,
    },
};
use schemars::JsonSchema;

use crate::entities::Exception;
use core::fmt;
use http::{header::HeaderName, HeaderMap, HeaderValue};
use schemars::{
    _serde_json::Value,
    schema::{InstanceType, Metadata, SchemaObject},
};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use smol_str::SmolStr;
use std::{
    collections::BTreeMap,
    hash::{Hash, Hasher},
};

#[derive(Debug, Hash, PartialEq, Clone)]
pub struct HeaderValueWrapper(HeaderValue);

mod header_value_ser {
    use super::*;
    use serde::{de, de::Visitor, Deserializer, Serializer};

    struct HeaderValueWrapperVisitor;

    impl<'de> Visitor<'de> for HeaderValueWrapperVisitor {
        type Value = HeaderValueWrapper;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            write!(formatter, "HTTP header value")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(HeaderValueWrapper(value.parse().map_err(|e| {
                de::Error::custom(format!("bad format: {} on {}", e, value))
            })?))
        }
    }

    impl<'de> Deserialize<'de> for HeaderValueWrapper {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.deserialize_str(HeaderValueWrapperVisitor)
        }
    }

    impl Serialize for HeaderValueWrapper {
        fn serialize<S>(
            &self,
            serializer: S,
        ) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
        where
            S: Serializer,
        {
            serializer.serialize_str(self.0.to_str().unwrap())
        }
    }
}

#[derive(Debug, Eq, Clone, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct HeaderMapWrapper(#[serde(with = "http_serde::header_map")] pub HeaderMap);

impl JsonSchema for HeaderMapWrapper {
    fn schema_name() -> String {
        "HttpHeaderMap".to_string()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        SchemaObject {
            metadata: Some(Box::new(Metadata {
                title: Some(String::from("HTTP Headers")),
                description: Some(String::from("Map of HTTP headers, where key is the header name. The headers value may be a single string or multiple strings.")),
                ..Default::default()
            })),
            instance_type: Some(InstanceType::Object.into()),
            ..Default::default()
        }
        .into()
    }
}

impl From<HeaderMap> for HeaderMapWrapper {
    fn from(map: HeaderMap) -> Self {
        HeaderMapWrapper(map)
    }
}

impl PartialEq for HeaderMapWrapper {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl Hash for HeaderMapWrapper {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for (k, v) in &self.0 {
            k.hash(state);
            v.hash(state);
        }
    }
}

impl HeaderMapWrapper {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
#[serde(transparent)]
pub struct MethodWrapper(#[serde(with = "http_serde::method")] pub http::Method);

impl ToString for MethodWrapper {
    fn to_string(&self) -> String {
        self.0.to_string()
    }
}

impl Default for MethodWrapper {
    fn default() -> Self {
        MethodWrapper(http::Method::GET)
    }
}

impl JsonSchema for MethodWrapper {
    fn schema_name() -> String {
        "HttpMethod".to_string()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        SchemaObject {
            metadata: Some(Box::new(Metadata {
                title: Some(String::from("HTTP Method")),
                ..Default::default()
            })),
            enum_values: Some(vec![
                Value::String(String::from("TRACE")),
                Value::String(String::from("PATCH")),
                Value::String(String::from("CONNECT")),
                Value::String(String::from("OPTIONS")),
                Value::String(String::from("HEAD")),
                Value::String(String::from("DELETE")),
                Value::String(String::from("PUT")),
                Value::String(String::from("POST")),
                Value::String(String::from("GET")),
            ]),
            instance_type: Some(InstanceType::String.into()),
            ..Default::default()
        }
        .into()
    }
}

impl From<http::Method> for MethodWrapper {
    fn from(map: http::Method) -> Self {
        MethodWrapper(map)
    }
}

#[serde_as]
#[derive(Debug, Eq, Clone, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct HeaderNameList(#[serde_as(as = "Vec<DisplayFromStr>")] pub Vec<HeaderName>);

impl JsonSchema for HeaderNameList {
    fn schema_name() -> String {
        format!("Array_of_HttpHeaderName")
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        SchemaObject {
            metadata: Some(Box::new(Metadata {
                title: Some(String::from("Array of HTTP Header Names")),
                description: Some(String::from("Array of HTTP Header Names")),
                ..Default::default()
            })),
            instance_type: Some(InstanceType::Array.into()),
            ..Default::default()
        }
        .into()
    }
}

impl From<Vec<HeaderName>> for HeaderNameList {
    fn from(map: Vec<HeaderName>) -> Self {
        HeaderNameList(map)
    }
}

impl PartialEq for HeaderNameList {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl Hash for HeaderNameList {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for k in &self.0 {
            k.hash(state);
        }
    }
}

impl HeaderNameList {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Debug, Default, Hash, Serialize, Deserialize, Eq, PartialEq, Clone, JsonSchema)]
pub struct ModifyHeaders {
    #[serde(default, skip_serializing_if = "HeaderMapWrapper::is_empty")]
    pub insert: HeaderMapWrapper,

    #[serde(default, skip_serializing_if = "HeaderMapWrapper::is_empty")]
    pub append: HeaderMapWrapper,

    #[serde(default)]
    pub remove: HeaderNameList,
}

impl ModifyHeaders {
    pub fn is_empty(&self) -> bool {
        HeaderMapWrapper::is_empty(&self.insert)
            && HeaderMapWrapper::is_empty(&self.append)
            && Vec::is_empty(&self.remove.0)
    }
}

#[derive(Debug, Hash, Serialize, Deserialize, Eq, PartialEq, Clone, JsonSchema)]
pub enum TrailingSlashModification {
    #[serde(rename = "keep")]
    Keep,

    #[serde(rename = "set")]
    Set,

    #[serde(rename = "unset")]
    Unset,
}

impl Default for TrailingSlashModification {
    fn default() -> Self {
        TrailingSlashModification::Keep
    }
}

#[derive(Default, Debug, Hash, Serialize, Deserialize, Eq, PartialEq, Clone, JsonSchema)]
pub struct RequestModifications {
    #[serde(default, skip_serializing_if = "ModifyHeaders::is_empty")]
    pub headers: ModifyHeaders,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<Vec<PathSegmentsModify>>,

    #[serde(rename = "trailing-slash", default, skip_serializing_if = "is_default")]
    pub trailing_slash: TrailingSlashModification,

    #[serde(default, rename = "query-params", skip_serializing_if = "is_default")]
    pub query_params: ModifyQuery,
}

#[derive(Debug, Hash, Serialize, Deserialize, Eq, PartialEq, Clone, JsonSchema)]
#[serde(tag = "strategy")]
pub enum ModifyQueryStrategy {
    #[serde(rename = "keep")]
    Keep {
        #[serde(default)]
        remove: Vec<SmolStr>,
    },

    #[serde(rename = "remove")]
    Remove {
        #[serde(default)]
        keep: Vec<SmolStr>,
    },
}

impl Default for ModifyQueryStrategy {
    fn default() -> Self {
        ModifyQueryStrategy::Keep { remove: vec![] }
    }
}

#[derive(Default, Debug, Hash, Serialize, Deserialize, Eq, PartialEq, Clone, JsonSchema)]
pub struct ModifyQuery {
    #[serde(default, flatten)]
    pub strategy: ModifyQueryStrategy,

    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub set: BTreeMap<SmolStr, SmolStr>,
}

#[derive(Default, Debug, Hash, Serialize, Deserialize, PartialEq, Clone, JsonSchema)]
pub struct ResponseModifications {
    #[serde(default, skip_serializing_if = "ModifyHeaders::is_empty")]
    pub headers: ModifyHeaders,
}

#[derive(Debug, Hash, Serialize, Deserialize, PartialEq, Clone, JsonSchema)]
pub struct OnResponse {
    pub when: ResponseConditions,
    pub modifications: ResponseModifications,
}

#[derive(Debug, Hash, Serialize, Deserialize, PartialEq, Clone, JsonSchema)]
pub struct ResponseConditions {
    #[serde(rename = "status-code")]
    pub status_code: StatusCodeRange,
}

#[derive(Debug, Hash, Serialize, Deserialize, PartialEq, Clone, JsonSchema)]
pub struct Rule {
    pub filter: Filter,
    #[serde(flatten)]
    pub action: Action,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profiles: Option<Vec<ProfileName>>,
}

#[derive(Debug, Hash, Serialize, Deserialize, PartialEq, Clone, Copy, JsonSchema)]
pub enum TrailingSlashFilterRule {
    #[serde(rename = "require")]
    Require,

    #[serde(rename = "allow")]
    Allow,

    #[serde(rename = "deny")]
    Deny,
}

impl Default for TrailingSlashFilterRule {
    fn default() -> Self {
        TrailingSlashFilterRule::Allow
    }
}

impl TrailingSlashFilterRule {
    fn is_default(&self) -> bool {
        self == &Default::default()
    }
}

#[derive(Debug, Hash, Serialize, Deserialize, PartialEq, Clone, JsonSchema)]
pub struct Filter {
    pub path: MatchingPath,

    #[serde(
        default,
        rename = "query-params",
        skip_serializing_if = "QueryMatcher::is_empty"
    )]
    pub query_params: QueryMatcher,

    #[serde(default, skip_serializing_if = "MethodMatcher::is_all")]
    pub methods: MethodMatcher,

    #[serde(
        rename = "trailing-slash",
        default,
        skip_serializing_if = "TrailingSlashFilterRule::is_default"
    )]
    pub trailing_slash: TrailingSlashFilterRule,
}

#[derive(Debug, Hash, Serialize, Deserialize, PartialEq, Clone, JsonSchema)]
#[serde(tag = "action")]
pub enum Action {
    /// process by the handler
    #[serde(rename = "invoke")]
    Invoke {
        #[serde(
            default,
            rename = "modify-request",
            skip_serializing_if = "Option::is_none"
        )]
        modify_request: Option<RequestModifications>,

        #[serde(default, rename = "on-response", skip_serializing_if = "Vec::is_empty")]
        on_response: Vec<OnResponse>,

        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        rescue: Vec<RescueItem>,
    },

    /// stop rules processing and move on to the next handler
    #[serde(rename = "next-handler")]
    NextHandler,

    /// finish the whole handlers chain and move to finalizer
    #[serde(rename = "throw")]
    Throw {
        exception: Exception,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        data: BTreeMap<SmolStr, SmolStr>,
    },

    /// finish the whole processing chain with the desired response
    #[serde(rename = "respond")]
    Respond {
        #[serde(rename = "static-response")]
        static_response: Container<StaticResponse, StaticResponseName>,

        #[serde(
            rename = "status-code",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        status_code: Option<StatusCode>,

        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        data: BTreeMap<SmolStr, SmolStr>,

        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        rescue: Vec<RescueItem>,
    },
}

impl Action {
    pub fn modify_request(&self) -> Option<&RequestModifications> {
        match self {
            Action::Invoke { modify_request, .. } => modify_request.as_ref(),
            Action::NextHandler => None,
            Action::Throw { .. } => None,
            Action::Respond { .. } => None,
        }
    }

    pub fn on_response(&self) -> Vec<&OnResponse> {
        match self {
            Action::Invoke { on_response, .. } => on_response.iter().collect(),
            _ => vec![],
        }
    }

    pub fn rescue(&self) -> Option<&Vec<RescueItem>> {
        match self {
            Action::Invoke { rescue, .. } | Action::Respond { rescue, .. } => Some(rescue),
            _ => None,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::config_core::{MatchQuerySingleValue, MatchQueryValue, QueryMatcher};
    use maplit::btreemap;

    #[test]
    pub fn test_method_schema() {
        serde_json::to_string_pretty(&schemars::schema_for!(MethodWrapper)).unwrap();
    }

    #[test]
    pub fn test_status_code_schema() {
        serde_json::to_string_pretty(&schemars::schema_for!(HeaderMapWrapper)).unwrap();
    }

    #[test]
    pub fn test_modify_headers_schema() {
        serde_json::to_string_pretty(&schemars::schema_for!(ModifyHeaders)).unwrap();
    }

    #[test]
    pub fn test_modify_headers() {
        let mut insert_headers = HeaderMap::new();
        insert_headers.insert("X-Amz-1", "1".parse().unwrap());
        assert_eq!(
            ModifyHeaders {
                insert: HeaderMapWrapper(insert_headers),
                append: Default::default(),
                remove: vec!["X-Amz-2".parse().unwrap()].into()
            },
            serde_yaml::from_str::<ModifyHeaders>(
                r#"
---
insert: 
  X-Amz-1: "1"
remove: 
  - X-Amz-2
"#
            )
            .unwrap()
        );
    }

    #[test]
    pub fn test_query_matcher() {
        let matcher = QueryMatcher {
            inner: btreemap! {
                "q1".into() => Some(MatchQueryValue::Single(MatchQuerySingleValue::MayBeAnyMultipleSegments)),
                "q2".into() => Some(MatchQueryValue::Single(MatchQuerySingleValue::AnySingleSegment)),
                "q3".into() => Some(MatchQueryValue::Single(MatchQuerySingleValue::Exact("v1".into()))),
                "q4".into() => Some(MatchQueryValue::Single(MatchQuerySingleValue::Regex(".+".parse().unwrap())))
            },
        };
        assert_eq!(
            Filter {
                path: MatchingPath::Root,
                query_params: matcher,
                methods: Default::default(),
                trailing_slash: Default::default()
            },
            serde_yaml::from_str::<Filter>(
                r#"
---
path: []
query: 
  q1: "*"
  q2: "?"
  q3: "v1"
  q4: "/.+/"
"#
            )
            .unwrap()
        );
    }
}
