use std::fmt;

use regex::Regex;
use serde::de::{IntoDeserializer, SeqAccess, Visitor};
use serde::ser::SerializeSeq;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

use crate::config_core::path_segment::{UrlPathSegment, UrlPathSegmentVisitor};
use std::hash::{Hash, Hasher};

pub const ANY_SEGMENTS_MATCH_STR: &str = "*";
pub const ANY_STR: &str = "?";
// pub const REF_STR: &str = "$";

#[derive(Debug, Clone, PartialEq, Hash)]
pub enum MatchPathSegment {
    Single(MatchPathSingleSegment),
    Choice(Vec<UrlPathSegment>),
}

impl Serialize for MatchPathSegment {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        match self {
            MatchPathSegment::Single(single) => single.serialize(serializer),
            MatchPathSegment::Choice(s) => {
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
pub enum MatchPathSingleSegment {
    Any,
    Exact(UrlPathSegment),
    Regex(Regex),
}

impl MatchPathSingleSegment {
    pub fn is_any_single_path_segment(&self) -> bool {
        matches!(self, MatchPathSingleSegment::Any)
    }

    pub fn single_segment(&self) -> Option<&UrlPathSegment> {
        match self {
            MatchPathSingleSegment::Exact(segment) => Some(segment),
            _ => None,
        }
    }

    pub fn single_regex(&self) -> Option<&Regex> {
        match self {
            MatchPathSingleSegment::Regex(regex) => Some(regex),
            _ => None,
        }
    }
}

impl Hash for MatchPathSingleSegment {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            MatchPathSingleSegment::Any => {
                state.write(&[1]);
            }
            MatchPathSingleSegment::Exact(exact) => {
                state.write(&[2]);
                exact.hash(state);
            }
            MatchPathSingleSegment::Regex(regex) => {
                state.write(&[3]);
                regex.as_str().hash(state);
            }
        }
    }
}

impl PartialEq for MatchPathSingleSegment {
    fn eq(&self, other: &Self) -> bool {
        use MatchPathSingleSegment::*;

        match (self, other) {
            (Any, Any) => true,
            (Exact(l), Exact(r)) => l.eq(r),
            (Regex(l), Regex(r)) => l.as_str().eq(r.as_str()),
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Hash)]
pub enum MatchingPath {
    // /
    Root,
    // *
    Wildcard,
    // A / B / C
    Strict(Vec<MatchPathSegment>),
    // Left / * / Right
    LeftWildcardRight(Vec<MatchPathSegment>, Vec<MatchPathSegment>),
    // Left / *
    LeftWildcard(Vec<MatchPathSegment>),
    // * / Right
    WildcardRight(Vec<MatchPathSegment>),
}

impl MatchingPath {
    pub fn is_root(&self) -> bool {
        matches!(self, MatchingPath::Root)
    }
    pub fn is_wildcard(&self) -> bool {
        matches!(self, MatchingPath::Wildcard)
    }

    pub fn simple(&self) -> Option<&Vec<MatchPathSegment>> {
        if let MatchingPath::Strict(simple) = self {
            Some(simple)
        } else {
            None
        }
    }

    pub fn left_wildcard(&self) -> Option<&Vec<MatchPathSegment>> {
        if let MatchingPath::LeftWildcard(left) = self {
            Some(left)
        } else {
            None
        }
    }

    pub fn left_wildcard_right(&self) -> Option<(&Vec<MatchPathSegment>, &Vec<MatchPathSegment>)> {
        if let MatchingPath::LeftWildcardRight(left, right) = self {
            Some((left, right))
        } else {
            None
        }
    }

    pub fn wildcard_right(&self) -> Option<&Vec<MatchPathSegment>> {
        if let MatchingPath::WildcardRight(right) = self {
            Some(right)
        } else {
            None
        }
    }
}

impl Serialize for MatchingPath {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        match self {
            MatchingPath::Root => {
                let seq = serializer.serialize_seq(Some(0))?;
                seq.end()
            }
            MatchingPath::Wildcard => {
                let mut seq = serializer.serialize_seq(Some(1))?;
                seq.serialize_element(ANY_SEGMENTS_MATCH_STR)?;
                seq.end()
            }
            MatchingPath::Strict(segments) => {
                let mut seq = serializer.serialize_seq(Some(segments.len()))?;
                for element in segments {
                    seq.serialize_element(element)?;
                }
                seq.end()
            }
            MatchingPath::LeftWildcardRight(left, right) => {
                let mut seq = serializer.serialize_seq(Some(left.len() + right.len() + 1))?;
                for element in left {
                    seq.serialize_element(element)?;
                }
                seq.serialize_element(ANY_SEGMENTS_MATCH_STR)?;
                for element in right {
                    seq.serialize_element(element)?;
                }
                seq.end()
            }
            MatchingPath::LeftWildcard(left) => {
                let mut seq = serializer.serialize_seq(Some(left.len() + 1))?;
                for element in left {
                    seq.serialize_element(element)?;
                }
                seq.serialize_element(ANY_SEGMENTS_MATCH_STR)?;
                seq.end()
            }
            MatchingPath::WildcardRight(right) => {
                let mut seq = serializer.serialize_seq(Some(right.len() + 1))?;
                seq.serialize_element(ANY_SEGMENTS_MATCH_STR)?;
                for element in right {
                    seq.serialize_element(element)?;
                }
                seq.end()
            }
        }
    }
}

impl Serialize for MatchPathSingleSegment {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        match self {
            MatchPathSingleSegment::Any => serializer.serialize_str(ANY_STR),
            MatchPathSingleSegment::Exact(s) => serializer.serialize_str(s.as_str()),
            MatchPathSingleSegment::Regex(s) => {
                serializer.serialize_str(format!("/{}/", s).as_str())
            }
        }
    }
}

struct MatchPathSingleSegmentVisitor;

impl<'de> Visitor<'de> for MatchPathSingleSegmentVisitor {
    type Value = MatchPathSingleSegment;

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
            Ok(MatchPathSingleSegment::Any)
        } else if value.len() > 1 && value.starts_with('/') && value.ends_with('/') {
            let trimmed = value.get(1..value.len() - 1).unwrap();
            // regex
            match trimmed.parse() {
                Ok(r) => Ok(MatchPathSingleSegment::Regex(r)),
                Err(e) => Err(de::Error::custom(e)),
            }
        } else {
            match value.parse() {
                Ok(r) => Ok(MatchPathSingleSegment::Exact(r)),
                Err(e) => Err(de::Error::custom(e)),
            }
        }
    }
}

impl<'de> Deserialize<'de> for MatchPathSingleSegment {
    fn deserialize<D>(deserializer: D) -> Result<MatchPathSingleSegment, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(MatchPathSingleSegmentVisitor)
    }
}

struct MatchPathSegmentVisitor;

impl<'de> Visitor<'de> for MatchPathSegmentVisitor {
    type Value = MatchPathSegment;

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
        Ok(MatchPathSegment::Single(
            value
                .into_deserializer()
                .deserialize_str(MatchPathSingleSegmentVisitor)?,
        ))
    }

    fn visit_seq<V>(self, mut visitor: V) -> Result<Self::Value, V::Error>
    where
        V: SeqAccess<'de>,
    {
        let mut r = vec![];

        while let Some(item) = visitor.next_element::<String>()? {
            r.push(
                item.into_deserializer()
                    .deserialize_str(UrlPathSegmentVisitor)?,
            );
        }
        Ok(MatchPathSegment::Choice(r))
    }
}

impl<'de> Deserialize<'de> for MatchPathSegment {
    fn deserialize<D>(deserializer: D) -> Result<MatchPathSegment, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(MatchPathSegmentVisitor)
    }
}

struct MatchPathVisitor;

#[derive(Debug)]
enum MatchPathSegmentOrStar {
    Star,
    Segment(MatchPathSegment),
}

struct MatchPathSegmentOrStarVisitor;

impl<'de> Visitor<'de> for MatchPathSegmentOrStarVisitor {
    type Value = MatchPathSegmentOrStar;

    fn expecting(&self, formatter: &mut core::fmt::Formatter) -> fmt::Result {
        write!(formatter, "*  or match segment")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if value == ANY_SEGMENTS_MATCH_STR {
            Ok(MatchPathSegmentOrStar::Star)
        } else {
            let r = value
                .into_deserializer()
                .deserialize_str(MatchPathSegmentVisitor)?;
            Ok(MatchPathSegmentOrStar::Segment(r))
        }
    }

    fn visit_seq<V>(self, mut visitor: V) -> Result<Self::Value, V::Error>
    where
        V: SeqAccess<'de>,
    {
        let mut r = vec![];

        while let Some(item) = visitor.next_element::<String>()? {
            r.push(
                item.into_deserializer()
                    .deserialize_str(UrlPathSegmentVisitor)?,
            );
        }
        Ok(MatchPathSegmentOrStar::Segment(MatchPathSegment::Choice(r)))
    }
}

impl<'de> Deserialize<'de> for MatchPathSegmentOrStar {
    fn deserialize<D>(deserializer: D) -> Result<MatchPathSegmentOrStar, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(MatchPathSegmentOrStarVisitor)
    }
}

impl<'de> Visitor<'de> for MatchPathVisitor {
    type Value = MatchingPath;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "valid path FIXME")
    }

    #[allow(clippy::unnecessary_unwrap)]
    fn visit_seq<V>(self, mut visitor: V) -> Result<Self::Value, V::Error>
    where
        V: SeqAccess<'de>,
    {
        let mut left: Option<Vec<MatchPathSegment>> = None;
        let mut right: Option<Vec<MatchPathSegment>> = None;

        let mut is_left_active = true;
        let mut is_first = true;

        while let Some(elem) = visitor.next_element::<MatchPathSegmentOrStar>()? {
            is_first = false;
            match elem {
                MatchPathSegmentOrStar::Star => {
                    if !is_left_active {
                        return Err(de::Error::custom("`*` is allowed only once"));
                    }
                    is_left_active = false;
                }
                MatchPathSegmentOrStar::Segment(segment) => {
                    if is_left_active {
                        left.get_or_insert_with(Default::default).push(segment);
                    } else {
                        right.get_or_insert_with(Default::default).push(segment);
                    }
                }
            }
        }

        if is_first {
            return Ok(MatchingPath::Root);
        }
        if right.is_none() && left.is_none() && !is_left_active {
            return Ok(MatchingPath::Wildcard);
        }

        if is_left_active {
            Ok(MatchingPath::Strict(left.unwrap()))
        } else {
            //right
            if right.is_none() || right.as_ref().map(|r| r.is_empty()).unwrap_or(false) {
                Ok(MatchingPath::LeftWildcard(left.unwrap()))
            } else {
                // something on the right
                if left.is_none() || left.as_ref().map(|r| r.is_empty()).unwrap_or(false) {
                    Ok(MatchingPath::WildcardRight(right.unwrap()))
                } else {
                    Ok(MatchingPath::LeftWildcardRight(
                        left.unwrap(),
                        right.unwrap(),
                    ))
                }
            }
        }
    }
}

impl<'de> Deserialize<'de> for MatchingPath {
    fn deserialize<D>(deserializer: D) -> Result<MatchingPath, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(MatchPathVisitor)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn test_path_segment_deserialize() {
        const YAML: &str = r#"
---
- "?"
- "a"
- /.+/
- ["a", "b"]
"#;

        let mut parsed = serde_yaml::from_str::<Vec<MatchPathSegment>>(YAML)
            .unwrap()
            .into_iter();

        assert!(matches!(
            parsed.next().unwrap(),
            MatchPathSegment::Single(MatchPathSingleSegment::Any)
        ));
        assert_eq!(
            MatchPathSegment::Single(MatchPathSingleSegment::Exact("a".parse().unwrap())),
            parsed.next().unwrap()
        );
        assert_eq!(
            MatchPathSegment::Single(MatchPathSingleSegment::Regex(r".+".parse().unwrap())),
            parsed.next().unwrap()
        );

        assert_eq!(
            MatchPathSegment::Choice(vec!["a".parse().unwrap(), "b".parse().unwrap()]),
            parsed.next().unwrap()
        );
    }

    #[test]
    pub fn test_path_segment_serialize() {
        assert_eq!(
            "---\n\"?\"",
            serde_yaml::to_string(&MatchPathSegment::Single(MatchPathSingleSegment::Any)).unwrap()
        );
        assert_eq!(
            "---\nseg",
            serde_yaml::to_string(&MatchPathSegment::Single(MatchPathSingleSegment::Exact(
                "seg".parse().unwrap()
            )))
            .unwrap()
        );
        assert_eq!(
            "---\n\"/[a-z]{1,}/\"",
            serde_yaml::to_string(&MatchPathSegment::Single(MatchPathSingleSegment::Regex(
                "[a-z]{1,}".parse().unwrap()
            )))
            .unwrap()
        );
        assert_eq!(
            "---\n- seg\n- seg2",
            serde_yaml::to_string(&MatchPathSegment::Choice(vec![
                "seg".parse().unwrap(),
                "seg2".parse().unwrap()
            ]))
            .unwrap()
        );
    }

    #[test]
    pub fn test_path_segment_error() {
        assert!(serde_yaml::from_str::<MatchPathSegment>("\"..\"").is_err());
        assert!(serde_yaml::from_str::<MatchPathSegment>("\".\"").is_err());
        assert!(serde_yaml::from_str::<MatchPathSegment>("\"*\"").is_err());
        assert!(serde_yaml::from_str::<MatchPathSegment>("\"\"").is_err());
        assert!(serde_yaml::from_str::<MatchPathSegment>("\"a\"").is_ok());
    }

    #[test]
    pub fn test_path_deserialize() {
        const YAML: &str = r#"
---
- []
- ["*"]
- ["a"]
- ["a", "b"]
- ["a", "b", "*"]
- ["a", "b", "*", "/.+\\.(jpg|gif|png)/"]
- ["*", "c"]
- ["?", "*"]
- [["a", "b"], "*"]
"#;
        let mut parsed = serde_yaml::from_str::<Vec<MatchingPath>>(YAML)
            .unwrap()
            .into_iter();

        assert!(parsed.next().unwrap().is_root());

        assert!(parsed.next().unwrap().is_wildcard());
        assert_eq!(
            MatchingPath::Strict(vec![MatchPathSegment::Single(
                MatchPathSingleSegment::Exact("a".parse().unwrap())
            )]),
            parsed.next().unwrap()
        );
        assert_eq!(
            MatchingPath::Strict(vec![
                MatchPathSegment::Single(MatchPathSingleSegment::Exact("a".parse().unwrap())),
                MatchPathSegment::Single(MatchPathSingleSegment::Exact("b".parse().unwrap()))
            ]),
            parsed.next().unwrap()
        );
        assert_eq!(
            MatchingPath::LeftWildcard(vec![
                MatchPathSegment::Single(MatchPathSingleSegment::Exact("a".parse().unwrap())),
                MatchPathSegment::Single(MatchPathSingleSegment::Exact("b".parse().unwrap()))
            ]),
            parsed.next().unwrap()
        );

        assert_eq!(
            MatchingPath::LeftWildcardRight(
                vec![
                    MatchPathSegment::Single(MatchPathSingleSegment::Exact("a".parse().unwrap())),
                    MatchPathSegment::Single(MatchPathSingleSegment::Exact("b".parse().unwrap()))
                ],
                vec![MatchPathSegment::Single(MatchPathSingleSegment::Regex(
                    r#".+\.(jpg|gif|png)"#.parse().unwrap()
                ))],
            ),
            parsed.next().unwrap()
        );

        assert_eq!(
            MatchingPath::WildcardRight(vec![MatchPathSegment::Single(
                MatchPathSingleSegment::Exact("c".parse().unwrap())
            )]),
            parsed.next().unwrap()
        );

        assert_eq!(
            MatchingPath::LeftWildcard(vec![MatchPathSegment::Single(MatchPathSingleSegment::Any)]),
            parsed.next().unwrap()
        );

        assert_eq!(
            MatchingPath::LeftWildcard(vec![MatchPathSegment::Choice(vec![
                "a".parse().unwrap(),
                "b".parse().unwrap()
            ])]),
            parsed.next().unwrap()
        );
    }

    #[test]
    pub fn test_path_error() {
        assert!(serde_yaml::from_str::<MatchingPath>("[\"*\", \"*\"]").is_err());
        assert!(serde_yaml::from_str::<MatchingPath>("[\"*\", a, \"*\"]").is_err());
        assert!(serde_yaml::from_str::<MatchingPath>("[a, \"*\", b, \"*\", c]").is_err());
    }
}
