use std::fmt;

use serde::de::{IntoDeserializer, SeqAccess, Visitor};
use serde::ser::SerializeSeq;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use url::Url;

use crate::rewrite::{PathSegmentRewrite, PathSegmentRewriteVisitor};

#[derive(Debug, Hash, Clone, Eq, PartialEq)]
pub enum RedirectTo {
    AbsoluteUrl(Url),
    WithBaseUrl(Url, Vec<PathSegmentRewrite>),
    Segments(Vec<PathSegmentRewrite>),
    Root,
}

struct RedirectToItemVisitor;

impl<'de> Visitor<'de> for RedirectToItemVisitor {
    type Value = RedirectTo;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("URL string, or array with segments, optionally starting with base URL")
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
                let r = first
                    .into_deserializer()
                    .deserialize_str(PathSegmentRewriteVisitor)?;
                vec.push(r);
            }
        } else {
            return Ok(RedirectTo::Root);
        }

        while let Some(elem) = visitor.next_element::<String>()? {
            let r = elem
                .into_deserializer()
                .deserialize_str(PathSegmentRewriteVisitor)?;
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

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
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

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Redirect {
    #[serde(default)]
    redirect_type: RedirectType,
    to: RedirectTo,
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::path_segment::UrlPathSegmentOrQueryPart;

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
                to: RedirectTo::Segments(vec![
                    "a".parse::<UrlPathSegmentOrQueryPart>().unwrap().into(),
                    "b".parse::<UrlPathSegmentOrQueryPart>().unwrap().into()
                ]),
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

        assert_eq!(
            Redirect {
                redirect_type: RedirectType::Temporary,
                to: RedirectTo::WithBaseUrl(
                    "https://google.com".parse().unwrap(),
                    vec![PathSegmentRewrite::Reference(1)]
                ),
            },
            serde_yaml::from_str(
                r#"
---
to: ["https://google.com", "$1"]
"#
            )
            .unwrap()
        );

        assert_eq!(
            Redirect {
                redirect_type: RedirectType::Temporary,
                to: RedirectTo::Segments(vec![
                    PathSegmentRewrite::Reference(1),
                    "a".parse::<UrlPathSegmentOrQueryPart>().unwrap().into(),
                    PathSegmentRewrite::Reference(2),
                ]),
            },
            serde_yaml::from_str(
                r#"
---
to: ["$1", "a", "$2"]
"#
            )
            .unwrap()
        );
    }

    #[test]
    pub fn test_serialize() {
        assert_eq!(
            r#"---
redirect_type: temporary
to: []"#,
            serde_yaml::to_string(&Redirect {
                redirect_type: RedirectType::Temporary,
                to: RedirectTo::Root,
            })
            .unwrap()
        );

        assert_eq!(
            r#"---
redirect_type: permanent
to: "https://example.com/""#,
            serde_yaml::to_string(&Redirect {
                redirect_type: RedirectType::Permanent,
                to: RedirectTo::AbsoluteUrl("https://example.com".parse().unwrap()),
            })
            .unwrap()
        );

        assert_eq!(
            r#"---
redirect_type: temporary
to:
  - "https://example.com/"
  - $1
  - b"#,
            serde_yaml::to_string(&Redirect {
                redirect_type: RedirectType::Temporary,
                to: RedirectTo::WithBaseUrl(
                    "https://example.com".parse().unwrap(),
                    vec![
                        PathSegmentRewrite::Reference(1),
                        PathSegmentRewrite::Single("b".parse().unwrap())
                    ],
                ),
            })
            .unwrap()
        );

        assert_eq!(
            r#"---
redirect_type: temporary
to:
  - b
  - $1"#,
            serde_yaml::to_string(&Redirect {
                redirect_type: RedirectType::Temporary,
                to: RedirectTo::Segments(vec![
                    PathSegmentRewrite::Single("b".parse().unwrap()),
                    PathSegmentRewrite::Reference(1),
                ]),
            })
            .unwrap()
        );
    }
}
