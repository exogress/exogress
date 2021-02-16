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
use url::Url;

#[derive(Debug, Hash, Clone, Eq, PartialEq)]
pub enum RedirectTo {
    AbsoluteUrl(Url),
    WithBaseUrl(Url, Vec<PathSegmentsModify>),
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
                    "URL, or array of path segments, optiionally starting from schema://url",
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
        match Url::parse(value) {
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
            if let Ok(url) = Url::parse(first.as_str()) {
                first_url = Some(url);
            } else {
                vec.push(Deserialize::deserialize(first.into_deserializer())?);
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

impl RedirectTo {
    pub fn to_destiation_string(&self) -> String {
        match self {
            RedirectTo::AbsoluteUrl(url) => url.to_string(),
            RedirectTo::Root => "/".to_string(),
            RedirectTo::WithBaseUrl(base_url, segments) => {
                let mut url = base_url.clone();
                for segment in segments {
                    url.path_segments_mut().unwrap().push(segment.as_str());
                }
                url.to_string()
            }
            RedirectTo::Segments(segments) => {
                let mut url = Url::parse("http://base").unwrap();
                for segment in segments {
                    url.path_segments_mut().unwrap().push(segment.as_str());
                }
                url.path().to_string()
            }
        }
    }
}

impl Serialize for RedirectTo {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        match self {
            RedirectTo::AbsoluteUrl(url) => serializer.serialize_str(url.as_str()),
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

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, JsonSchema)]
pub struct Redirect {
    #[serde(default)]
    redirect_type: RedirectType,
    to: RedirectTo,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn test_schema() {
        serde_json::to_string_pretty(&schemars::schema_for!(RedirectTo)).unwrap();
    }

    #[test]
    pub fn test_deserialize() {
        assert_eq!(
            Redirect {
                redirect_type: RedirectType::Temporary,
                to: RedirectTo::Root,
            },
            serde_yaml::from_str(
                r#"
---
to: []
"#
            )
            .unwrap()
        );

        assert_eq!(
            Redirect {
                redirect_type: RedirectType::Permanent,
                to: RedirectTo::Segments(vec!["a".parse().unwrap(), "b".parse().unwrap(),]),
            },
            serde_yaml::from_str(
                r#"
---
redirect_type: permanent
to: ["a", "b"]
"#
            )
            .unwrap()
        );

        assert_eq!(
            Redirect {
                redirect_type: RedirectType::Temporary,
                to: RedirectTo::AbsoluteUrl("https://google.com".parse().unwrap()),
            },
            serde_yaml::from_str(
                r#"
---
to: "https://google.com"
"#
            )
            .unwrap()
        );

        assert_eq!(
            Redirect {
                redirect_type: RedirectType::Temporary,
                to: RedirectTo::AbsoluteUrl("https://google.com".parse().unwrap()),
            },
            serde_yaml::from_str(
                r#"
---
to: "https://google.com"
"#
            )
            .unwrap()
        );

        //         assert_eq!(
        //             Redirect {
        //                 redirect_type: RedirectType::Temporary,
        //                 to: RedirectTo::WithBaseUrl(
        //                     "https://google.com".parse().unwrap(),
        //                     vec![PathSegmentRewrite::Reference(1)]
        //                 ),
        //             },
        //             serde_yaml::from_str(
        //                 r#"
        // ---
        // to: ["https://google.com", "$1"]
        // "#
        //             )
        //             .unwrap()
        //         );

        //         assert_eq!(
        //             Redirect {
        //                 redirect_type: RedirectType::Temporary,
        //                 to: RedirectTo::Segments(vec![
        //                     PathSegmentRewrite::Reference(1),
        //                     "a".parse::<UrlPathSegment>().unwrap().into(),
        //                     PathSegmentRewrite::Reference(2),
        //                 ]),
        //             },
        //             serde_yaml::from_str(
        //                 r#"
        // ---
        // to: ["$1", "a", "$2"]
        // "#
        //             )
        //             .unwrap()
        //         );
    }

    #[test]
    pub fn test_serialize() {
        assert_eq!(
            r#"---
redirect_type: temporary
to: []
"#,
            serde_yaml::to_string(&Redirect {
                redirect_type: RedirectType::Temporary,
                to: RedirectTo::Root,
            })
            .unwrap()
        );

        assert_eq!(
            r#"---
redirect_type: permanent
to: "https://example.com/"
"#,
            serde_yaml::to_string(&Redirect {
                redirect_type: RedirectType::Permanent,
                to: RedirectTo::AbsoluteUrl("https://example.com".parse().unwrap()),
            })
            .unwrap()
        );

        //         assert_eq!(
        //             r#"---
        // redirect_type: temporary
        // to:
        //   - "https://example.com/"
        //   - $1
        //   - b"#,
        //             serde_yaml::to_string(&Redirect {
        //                 redirect_type: RedirectType::Temporary,
        //                 to: RedirectTo::WithBaseUrl(
        //                     "https://example.com".parse().unwrap(),
        //                     vec![
        //                         PathSegmentRewrite::Reference(1),
        //                         PathSegmentRewrite::Single("b".parse().unwrap())
        //                     ],
        //                 ),
        //             })
        //             .unwrap()
        //         );

        //         assert_eq!(
        //             r#"---
        // redirect_type: temporary
        // to:
        //   - b
        //   - $1"#,
        //             serde_yaml::to_string(&Redirect {
        //                 redirect_type: RedirectType::Temporary,
        //                 to: RedirectTo::Segments(vec![
        //                     PathSegmentRewrite::Single("b".parse().unwrap()),
        //                     PathSegmentRewrite::Reference(1),
        //                 ]),
        //             })
        //             .unwrap()
        //         );
    }
}
