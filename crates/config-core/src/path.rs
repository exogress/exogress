use std::fmt;

use regex::Regex;
use serde::de::{IntoDeserializer, SeqAccess, Visitor};
use serde::ser::SerializeSeq;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

use crate::path_segment::UrlPathSegmentOrQueryPart;
use std::hash::{Hash, Hasher};

pub const ANY_SEGMENTS_MATCH_STR: &str = "*";
pub const ANY_STR: &str = "?";
// pub const REF_STR: &str = "$";

#[derive(Debug, Clone)]
pub enum MatchPathSegment {
    Any,
    Exact(UrlPathSegmentOrQueryPart),
    Regex(Regex),
    // Choice(Vec<UrlPathSegmentOrQueryPart>),
}

impl Hash for MatchPathSegment {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            MatchPathSegment::Any => {
                state.write(&[1]);
            }
            MatchPathSegment::Exact(exact) => {
                state.write(&[2]);
                exact.hash(state);
            }
            MatchPathSegment::Regex(regex) => {
                state.write(&[3]);
                regex.as_str().hash(state);
            } // MatchPathSegment::Choice(choice) => {
              //     state.write(&[4]);
              //     choice.hash(state);
              // }
        }
    }
}

impl PartialEq for MatchPathSegment {
    fn eq(&self, other: &Self) -> bool {
        use MatchPathSegment::*;

        match (self, other) {
            (Any, Any) => true,
            (Exact(l), Exact(r)) => l.eq(r),
            (Regex(l), Regex(r)) => l.as_str().eq(r.as_str()),
            // (Choice(l), Choice(r)) => l.eq(r),
            _ => false,
        }
    }
}

impl MatchPathSegment {
    pub fn is_any_single_path_segment(&self) -> bool {
        matches!(self, MatchPathSegment::Any)
    }

    pub fn single_segment(&self) -> Option<&UrlPathSegmentOrQueryPart> {
        match self {
            MatchPathSegment::Exact(segment) => Some(segment),
            _ => None,
        }
    }

    pub fn single_regex(&self) -> Option<&Regex> {
        match self {
            MatchPathSegment::Regex(regex) => Some(regex),
            _ => None,
        }
    }

    // pub fn choice(&self) -> Option<&Vec<UrlPathSegmentOrQueryPart>> {
    //     match self {
    //         MatchPathSegment::Choice(segments) => Some(segments),
    //         _ => None,
    //     }
    // }

    pub fn is_match(&self, s: &str) -> bool {
        match self {
            MatchPathSegment::Any => true,
            MatchPathSegment::Exact(segment) => segment.as_ref() == s,
            MatchPathSegment::Regex(re) => re.is_match(s),
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

impl Serialize for MatchPathSegment {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        match self {
            MatchPathSegment::Any => serializer.serialize_str(ANY_STR),
            MatchPathSegment::Exact(s) => serializer.serialize_str(s.as_str()),
            MatchPathSegment::Regex(s) => serializer.serialize_str(format!("/{}/", s).as_str()),
            // MatchPathSegment::Choice(s) => {
            //     let mut seq = serializer.serialize_seq(Some(s.len()))?;
            //     for element in s {
            //         seq.serialize_element(element.as_str())?;
            //     }
            //     seq.end()
            // }
        }
    }
}

struct MatchPathSegmentVisitor;

impl<'de> Visitor<'de> for MatchPathSegmentVisitor {
    type Value = MatchPathSegment;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(
            "single path segment \"s\", multiple segments [\"s1\", \"s1\"], \"?\" or \"*\"",
        )
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if value == ANY_STR {
            Ok(MatchPathSegment::Any)
        } else if value.len() > 1 && value.starts_with('/') && value.ends_with('/') {
            let trimmed = value.get(1..value.len() - 1).unwrap();
            // regex
            match trimmed.parse() {
                Ok(r) => Ok(MatchPathSegment::Regex(r)),
                Err(e) => Err(de::Error::custom(e)),
            }
        } else {
            match value.parse() {
                Ok(r) => Ok(MatchPathSegment::Exact(r)),
                Err(e) => Err(de::Error::custom(e)),
            }
        }
    }

    // fn visit_seq<V>(self, mut visitor: V) -> Result<Self::Value, V::Error>
    // where
    //     V: SeqAccess<'de>,
    // {
    //     let mut vec = Vec::new();
    //
    //     while let Some(elem) = visitor.next_element::<String>()? {
    //         match elem.parse() {
    //             Ok(r) => {
    //                 vec.push(r);
    //             }
    //             Err(e) => {
    //                 return Err(de::Error::custom(e));
    //             }
    //         }
    //     }
    //
    //     Ok(MatchPathSegment::Choice(vec))
    // }
}

impl<'de> Deserialize<'de> for MatchPathSegment {
    fn deserialize<D>(deserializer: D) -> Result<MatchPathSegment, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(MatchPathSegmentVisitor)
    }
}

struct PathVisitor;

impl<'de> Visitor<'de> for PathVisitor {
    type Value = MatchingPath;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("valid path FIXME")
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

        while let Some(elem) = visitor.next_element::<String>()? {
            is_first = false;
            if elem == ANY_SEGMENTS_MATCH_STR {
                if !is_left_active {
                    return Err(de::Error::custom("`*` is allowed only once"));
                }
                is_left_active = false;
            } else {
                let r = elem
                    .into_deserializer()
                    .deserialize_str(MatchPathSegmentVisitor)?;
                if is_left_active {
                    left.get_or_insert_with(Default::default).push(r);
                } else {
                    right.get_or_insert_with(Default::default).push(r);
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
        deserializer.deserialize_seq(PathVisitor)
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
"#;

        let mut parsed = serde_yaml::from_str::<Vec<MatchPathSegment>>(YAML)
            .unwrap()
            .into_iter();

        assert!(parsed.next().unwrap().is_any_single_path_segment());
        assert_eq!(
            MatchPathSegment::Exact("a".parse().unwrap()),
            parsed.next().unwrap()
        );
        assert_eq!(
            MatchPathSegment::Regex(r".+".parse().unwrap()),
            parsed.next().unwrap()
        );

        // assert_eq!(
        //     MatchPathSegment::Choice(vec!["a".parse().unwrap(), "b".parse().unwrap()]),
        //     parsed.next().unwrap()
        // );
    }

    #[test]
    pub fn test_path_segment_serialize() {
        assert_eq!(
            "---\n\"?\"",
            serde_yaml::to_string(&MatchPathSegment::Any).unwrap()
        );
        assert_eq!(
            "---\nseg",
            serde_yaml::to_string(&MatchPathSegment::Exact("seg".parse().unwrap())).unwrap()
        );
        assert_eq!(
            "---\n\"/[a-z]{1,}/\"",
            serde_yaml::to_string(&MatchPathSegment::Regex("[a-z]{1,}".parse().unwrap())).unwrap()
        );
        // assert_eq!(
        //     "---\n- seg\n- seg2",
        //     serde_yaml::to_string(&MatchPathSegment::Choice(vec![
        //         "seg".parse().unwrap(),
        //         "seg2".parse().unwrap()
        //     ]))
        //     .unwrap()
        // );
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
- ["a", "b", "*", "/.+(jpg|gif|png)/"]
- ["*", "c"]
"#;
        let mut parsed = serde_yaml::from_str::<Vec<MatchingPath>>(YAML)
            .unwrap()
            .into_iter();

        assert!(parsed.next().unwrap().is_root());

        assert!(parsed.next().unwrap().is_wildcard());
        assert_eq!(
            MatchingPath::Strict(vec![MatchPathSegment::Exact("a".parse().unwrap())]),
            parsed.next().unwrap()
        );
        assert_eq!(
            MatchingPath::Strict(vec![
                MatchPathSegment::Exact("a".parse().unwrap()),
                MatchPathSegment::Exact("b".parse().unwrap())
            ]),
            parsed.next().unwrap()
        );
        assert_eq!(
            MatchingPath::LeftWildcard(vec![
                MatchPathSegment::Exact("a".parse().unwrap()),
                MatchPathSegment::Exact("b".parse().unwrap())
            ]),
            parsed.next().unwrap()
        );

        assert_eq!(
            MatchingPath::LeftWildcardRight(
                vec![
                    MatchPathSegment::Exact("a".parse().unwrap()),
                    MatchPathSegment::Exact("b".parse().unwrap())
                ],
                vec![MatchPathSegment::Regex(
                    r#".+(jpg|gif|png)"#.parse().unwrap()
                )],
            ),
            parsed.next().unwrap()
        );

        assert_eq!(
            MatchingPath::WildcardRight(vec![MatchPathSegment::Exact("c".parse().unwrap())]),
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
