use std::fmt;

use crate::{
    config_core::path_modify::PathSegmentsModify,
    entities::schemars::{gen::SchemaGenerator, schema::Schema},
};
use schemars::{
    schema::{ArrayValidation, InstanceType, Metadata, SchemaObject, StringValidation},
    JsonSchema,
};
use serde::{
    de,
    de::{IntoDeserializer, SeqAccess, Visitor},
    ser::SerializeSeq,
    Deserialize, Deserializer, Serialize, Serializer,
};
use std::str::FromStr;
use url::Url;

#[derive(Debug, Hash, Clone, Eq, PartialEq)]
pub enum RedirectTo {
    AbsoluteUrl(http::Uri),
    WithBaseUrl(http::Uri, Vec<PathSegmentsModify>),
    Segments(Vec<PathSegmentsModify>),
    Root,
}

impl JsonSchema for RedirectTo {
    fn schema_name() -> String {
        "RedirectTo".to_string()
    }

    fn json_schema(gen: &mut SchemaGenerator) -> Schema {
        SchemaObject {
            metadata: Some(Box::new(Metadata {
                title: Some(String::from(
                    "URL, or array of path segments, optionally starting from schema://url",
                )),
                description: None,
                ..Default::default()
            })),
            instance_type: Some(vec![InstanceType::String, InstanceType::Array].into()),
            array: Some(Box::new(ArrayValidation {
                items: Some(vec![gen.subschema_for::<String>()].into()),
                ..Default::default()
            })),
            string: Some(Box::new(StringValidation {
                ..Default::default()
            })),
            ..Default::default()
        }
        .into()
    }
}

struct RedirectToItemVisitor;

impl<'de> Visitor<'de> for RedirectToItemVisitor {
    type Value = RedirectTo;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "URL string, or array with segments, optionally starting with base URL"
        )
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        match http::Uri::from_str(value) {
            Ok(url) => Ok(RedirectTo::AbsoluteUrl(url)),
            Err(e) => Err(de::Error::custom(e)),
        }
    }

    fn visit_seq<V>(self, mut visitor: V) -> Result<Self::Value, V::Error>
    where
        V: SeqAccess<'de>,
    {
        let mut vec = Vec::new();

        let mut first_url = None;

        if let Some(first) = visitor.next_element::<String>()? {
            let is_valid_url = Url::parse(first.as_str()).is_ok();

            if !is_valid_url {
                vec.push(Deserialize::deserialize(first.into_deserializer())?);
            } else if let Ok(url) = http::Uri::from_str(first.as_str()) {
                first_url = Some(url);
            } else {
                return Err(de::Error::custom("bad first segment"));
            }
        } else {
            return Ok(RedirectTo::Root);
        }

        while let Some(elem) = visitor.next_element::<String>()? {
            let r = Deserialize::deserialize(elem.into_deserializer())?;
            vec.push(r);
        }

        let r = match first_url {
            Some(first) => RedirectTo::WithBaseUrl(first, vec),
            None => RedirectTo::Segments(vec),
        };

        Ok(r)
    }
}

impl<'de> Deserialize<'de> for RedirectTo {
    fn deserialize<D>(deserializer: D) -> Result<RedirectTo, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(RedirectToItemVisitor)
    }
}

impl Serialize for RedirectTo {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        match self {
            RedirectTo::AbsoluteUrl(url) => serializer.serialize_str(url.to_string().as_str()),
            RedirectTo::Root => {
                let seq = serializer.serialize_seq(Some(0))?;
                seq.end()
            }
            RedirectTo::WithBaseUrl(base_url, segments) => {
                let mut seq = serializer.serialize_seq(Some(segments.len() + 1))?;
                seq.serialize_element(base_url.to_string().as_str())?;
                for element in segments {
                    seq.serialize_element(element)?;
                }
                seq.end()
            }
            RedirectTo::Segments(segments) => {
                let mut seq = serializer.serialize_seq(Some(segments.len()))?;
                for element in segments {
                    seq.serialize_element(element)?;
                }
                seq.end()
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, JsonSchema)]
pub enum RedirectType {
    #[serde(rename = "permanent")]
    Permanent,
    #[serde(rename = "temporary")]
    Temporary,
}

impl Default for RedirectType {
    fn default() -> Self {
        RedirectType::Temporary
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn test_schema() {
        serde_json::to_string_pretty(&schemars::schema_for!(RedirectTo)).unwrap();
    }
}
