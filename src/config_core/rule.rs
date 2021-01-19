use crate::config_core::catch::{Exception, RescueItem};
use crate::config_core::path::MatchingPath;
use crate::config_core::StatusCode;
use crate::entities::StaticResponseName;
use core::fmt;
use http::header::HeaderName;
use http::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use smol_str::SmolStr;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

#[derive(Debug, Hash, PartialEq, Clone)]
pub struct HeaderValueWrapper(HeaderValue);

mod header_value_ser {
    use super::*;
    use serde::de::Visitor;
    use serde::{de, Deserializer, Serializer};

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

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct HeaderMapWrapper(#[serde(with = "http_serde::header_map")] pub HeaderMap);

impl Hash for HeaderMapWrapper {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for (k, v) in &self.0 {
            k.hash(state);
            v.hash(state);
        }
    }
}

impl HeaderMapWrapper {
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[serde_as]
#[derive(Debug, Hash, Serialize, Deserialize, PartialEq, Clone)]
#[serde(deny_unknown_fields)]
pub struct ModifyHeaders {
    #[serde(default, skip_serializing_if = "HeaderMapWrapper::is_empty")]
    pub insert: HeaderMapWrapper,

    #[serde(default, skip_serializing_if = "HeaderMapWrapper::is_empty")]
    pub append: HeaderMapWrapper,

    #[serde_as(as = "Vec<DisplayFromStr>")]
    #[serde(default)]
    pub remove: Vec<HeaderName>,
}

#[derive(Debug, Hash, Serialize, Deserialize, PartialEq, Clone)]
#[serde(deny_unknown_fields)]
pub struct Modify {
    pub headers: ModifyHeaders,
}

#[derive(Debug, Hash, Serialize, Deserialize, PartialEq, Clone)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub filter: Filter,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modify: Option<Modify>,
    #[serde(skip_serializing_if = "Action::is_none", default = "default_action")]
    pub action: Action,
}

fn default_action() -> Action {
    Action::None
}

#[derive(Debug, Hash, Serialize, Deserialize, PartialEq, Clone, Copy)]
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

#[derive(Debug, Hash, Serialize, Deserialize, PartialEq, Clone)]
#[serde(deny_unknown_fields)]
pub struct Filter {
    pub path: MatchingPath,

    #[serde(
        rename = "trailing-slash",
        default,
        skip_serializing_if = "TrailingSlashFilterRule::is_default"
    )]
    pub trailing_slash: TrailingSlashFilterRule,
}

// #[derive(Debug, Hash, Eq, Serialize, Deserialize, PartialEq, Clone)]
// #[serde(deny_unknown_fields)]
// pub struct Modify {}

#[derive(Debug, Hash, Eq, Serialize, Deserialize, PartialEq, Clone)]
#[serde(deny_unknown_fields, tag = "kind")]
pub enum Action {
    /// process by the handler
    #[serde(rename = "invoke")]
    Invoke {
        #[serde(default)]
        rescue: Vec<RescueItem>,
    },

    /// stop rules processing and move on to the next handler
    #[serde(rename = "next-handler")]
    NextHandler,

    /// move on to the next rule. typically, combined with rewrite
    #[serde(rename = "none")]
    None,

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
        name: StaticResponseName,

        #[serde(rename = "status-code", default)]
        status_code: Option<StatusCode>,

        #[serde(default)]
        data: BTreeMap<SmolStr, SmolStr>,

        #[serde(default)]
        rescue: Vec<RescueItem>,
    },
}

impl Default for Action {
    fn default() -> Self {
        Action::None
    }
}

impl Action {
    fn is_none(&self) -> bool {
        matches!(self, Action::None)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn test_modify_headers() {
        let mut insert_headers = HeaderMap::new();
        insert_headers.insert("X-Amz-1", "1".parse().unwrap());
        assert_eq!(
            ModifyHeaders {
                insert: HeaderMapWrapper(insert_headers),
                append: Default::default(),
                remove: vec!["X-Amz-2".parse().unwrap()]
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
}
