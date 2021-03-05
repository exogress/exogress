use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use std::{collections::BTreeMap, fmt};

use regex::Regex;
use serde::{
    de,
    de::{IntoDeserializer, SeqAccess, Visitor},
    ser::SerializeSeq,
    Deserializer, Serializer,
};

use crate::{
    config_core::path::{ANY_SEGMENTS_MATCH_STR, ANY_STR},
    entities::schemars::{gen::SchemaGenerator, schema::Schema},
};
use schemars::schema::{ArrayValidation, InstanceType, SchemaObject, SubschemaValidation};
use std::hash::{Hash, Hasher};

#[derive(Serialize, Deserialize, Debug, Hash, Eq, PartialEq, Clone, JsonSchema)]
#[serde(transparent)]
pub struct QueryMatcher {
    pub inner: BTreeMap<SmolStr, Option<MatchQueryValue>>,
}

impl QueryMatcher {
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl Default for QueryMatcher {
    fn default() -> Self {
        QueryMatcher {
            inner: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum MatchQueryValue {
    Single(MatchQuerySingleValue),
    Choice(Vec<SmolStr>),
}

impl JsonSchema for MatchQueryValue {
    fn schema_name() -> String {
        "MatchQueryValue".to_string()
    }

    fn json_schema(gen: &mut SchemaGenerator) -> Schema {
        SchemaObject {
            instance_type: Some(vec![InstanceType::String, InstanceType::Array].into()),
            array: Some(Box::new(ArrayValidation {
                items: Some(gen.subschema_for::<SmolStr>().into()),
                ..Default::default()
            })),
            subschemas: Some(Box::new(SubschemaValidation {
                any_of: Some(vec![
                    gen.subschema_for::<MatchQuerySingleValue>(),
                    gen.subschema_for::<Vec<SmolStr>>(),
                ]),
                ..Default::default()
            })),
            ..Default::default()
        }
        .into()
    }
}

impl Serialize for MatchQueryValue {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        match self {
            MatchQueryValue::Single(single) => single.serialize(serializer),
            MatchQueryValue::Choice(s) => {
                let mut seq = serializer.serialize_seq(Some(s.len()))?;
                for element in s {
                    seq.serialize_element(element.as_str())?;
                }
                seq.end()
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum MatchQuerySingleValue {
    AnySingleSegment,
    MayBeAnyMultipleSegments,
    Exact(SmolStr),
    Regex(Box<Regex>),
}

impl JsonSchema for MatchQuerySingleValue {
    fn schema_name() -> String {
        "MatchQuerySingleValue".to_string()
    }

    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        SchemaObject {
            instance_type: Some(InstanceType::String.into()),
            ..Default::default()
        }
        .into()
    }
}

impl MatchQuerySingleValue {
    pub fn is_any_single_path_segment(&self) -> bool {
        matches!(self, MatchQuerySingleValue::AnySingleSegment)
    }

    pub fn is_may_be_multiple_path_segments(&self) -> bool {
        matches!(self, MatchQuerySingleValue::MayBeAnyMultipleSegments)
    }

    pub fn single_segment(&self) -> Option<&SmolStr> {
        match self {
            MatchQuerySingleValue::Exact(segment) => Some(segment),
            _ => None,
        }
    }

    pub fn single_regex(&self) -> Option<&Regex> {
        match self {
            MatchQuerySingleValue::Regex(regex) => Some(regex),
            _ => None,
        }
    }
}

impl Hash for MatchQuerySingleValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            MatchQuerySingleValue::AnySingleSegment => {
                state.write(&[1]);
            }
            MatchQuerySingleValue::MayBeAnyMultipleSegments => {
                state.write(&[2]);
            }
            MatchQuerySingleValue::Exact(exact) => {
                state.write(&[3]);
                exact.hash(state);
            }
            MatchQuerySingleValue::Regex(regex) => {
                state.write(&[4]);
                regex.as_str().hash(state);
            }
        }
    }
}

impl PartialEq for MatchQuerySingleValue {
    fn eq(&self, other: &Self) -> bool {
        use MatchQuerySingleValue::*;

        match (self, other) {
            (AnySingleSegment, AnySingleSegment) => true,
            (MayBeAnyMultipleSegments, MayBeAnyMultipleSegments) => true,
            (Exact(l), Exact(r)) => l.eq(r),
            (Regex(l), Regex(r)) => l.as_str().eq(r.as_str()),
            _ => false,
        }
    }
}

impl Eq for MatchQuerySingleValue {}

impl Serialize for MatchQuerySingleValue {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        match self {
            MatchQuerySingleValue::AnySingleSegment => serializer.serialize_str(ANY_STR),
            MatchQuerySingleValue::MayBeAnyMultipleSegments => {
                serializer.serialize_str(ANY_SEGMENTS_MATCH_STR)
            }
            MatchQuerySingleValue::Exact(s) => serializer.serialize_str(s.as_str()),
            MatchQuerySingleValue::Regex(s) => {
                serializer.serialize_str(format!("/{}/", s).as_str())
            }
        }
    }
}

struct MatchQuerySingleValueVisitor;

impl<'de> Visitor<'de> for MatchQuerySingleValueVisitor {
    type Value = MatchQuerySingleValue;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "single path segment \"s\", regex single segment \"\\.+\\\", \"?\" or \"*\"",
        )
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if value == ANY_STR {
            Ok(MatchQuerySingleValue::AnySingleSegment)
        } else if value == ANY_SEGMENTS_MATCH_STR {
            Ok(MatchQuerySingleValue::MayBeAnyMultipleSegments)
        } else if value.len() > 1 && value.starts_with('/') && value.ends_with('/') {
            let trimmed = value.get(1..value.len() - 1).unwrap();
            // regex
            match trimmed.parse() {
                Ok(r) => Ok(MatchQuerySingleValue::Regex(Box::new(r))),
                Err(e) => Err(de::Error::custom(e)),
            }
        } else {
            Ok(MatchQuerySingleValue::Exact(value.into()))
        }
    }
}

impl<'de> Deserialize<'de> for MatchQuerySingleValue {
    fn deserialize<D>(deserializer: D) -> Result<MatchQuerySingleValue, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(MatchQuerySingleValueVisitor)
    }
}

struct MatchQueryValueVisitor;

impl<'de> Visitor<'de> for MatchQueryValueVisitor {
    type Value = MatchQueryValue;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "single path segment \"s\", multiple segments [\"s1\", \"s1\"], \"?\" or \"*\"",
        )
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(MatchQueryValue::Single(
            value
                .into_deserializer()
                .deserialize_str(MatchQuerySingleValueVisitor)?,
        ))
    }

    fn visit_seq<V>(self, mut visitor: V) -> Result<Self::Value, V::Error>
    where
        V: SeqAccess<'de>,
    {
        let mut r = vec![];

        while let Some(item) = visitor.next_element::<String>()? {
            r.push(item.into());
        }
        Ok(MatchQueryValue::Choice(r))
    }
}

impl<'de> Deserialize<'de> for MatchQueryValue {
    fn deserialize<D>(deserializer: D) -> Result<MatchQueryValue, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(MatchQueryValueVisitor)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use maplit::btreemap;

    #[test]
    pub fn test_deserialize() {
        const YAML: &str = r#"
---
p1: v1
p2: "*"
p3: "?"
p4: ~
p5: ""
p6: 
p7: null
p8: /.+/
p9: ["a", "b", "c"]
"#;
        let parsed = serde_yaml::from_str::<QueryMatcher>(YAML).unwrap();

        assert_eq!(
            parsed,
            QueryMatcher {
                inner: btreemap! {
                    SmolStr::from("p1") =>  Some(MatchQueryValue::Single(MatchQuerySingleValue::Exact("v1".into()))),
                    SmolStr::from("p2") =>  Some(MatchQueryValue::Single(MatchQuerySingleValue::MayBeAnyMultipleSegments)),
                    SmolStr::from("p3") =>  Some(MatchQueryValue::Single(MatchQuerySingleValue::AnySingleSegment)),
                    SmolStr::from("p4") =>  None,
                    SmolStr::from("p5") =>  Some(MatchQueryValue::Single(MatchQuerySingleValue::Exact("".into()))),
                    SmolStr::from("p6") =>  None,
                    SmolStr::from("p7") =>  None,
                    SmolStr::from("p8") =>  Some(MatchQueryValue::Single(MatchQuerySingleValue::Regex(Box::new(".+".parse().unwrap())))),
                    SmolStr::from("p9") =>  Some(MatchQueryValue::Choice(vec!["a".into(), "b".into(), "c".into()]))
                }
            }
        );
    }
}
