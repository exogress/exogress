use core::fmt;
use http::Method;
use serde::de::{SeqAccess, Visitor};
use serde::ser::SerializeSeq;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum MethodMatcher {
    All,
    Exact(Vec<Method>),
}

impl MethodMatcher {
    pub fn is_all(&self) -> bool {
        matches!(self, &MethodMatcher::All)
    }

    pub fn is_match(&self, method: &http::Method) -> bool {
        match self {
            MethodMatcher::All => true,
            MethodMatcher::Exact(expected_method) => {
                expected_method.iter().any(|expected| expected == method)
            }
        }
    }
}

impl Default for MethodMatcher {
    fn default() -> Self {
        MethodMatcher::All
    }
}

impl Serialize for MethodMatcher {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        match self {
            MethodMatcher::All => serializer.serialize_str("*"),
            MethodMatcher::Exact(methods) => {
                let mut seq = serializer.serialize_seq(Some(methods.len()))?;
                for method in methods {
                    seq.serialize_element(&method.to_string())?;
                }
                seq.end()
            }
        }
    }
}

pub struct MethodMatcherVisitor;

impl<'de> Visitor<'de> for MethodMatcherVisitor {
    type Value = MethodMatcher;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "list of HTTP methods or \"*\"")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut v: Vec<Method> = Vec::new();

        while let Some(item) = seq.next_element::<String>()? {
            v.push(item.parse().expect("FIXME"));
        }

        Ok(MethodMatcher::Exact(v))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if value == "*" {
            Ok(MethodMatcher::All)
        } else {
            Err(de::Error::custom("expected only \"*\" as a string"))
        }
    }
}

impl<'de> Deserialize<'de> for MethodMatcher {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(MethodMatcherVisitor)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn test_parsing_list() {
        const YAML: &str = r#"---
- GET
- HEAD
"#;
        let parsed = serde_yaml::from_str::<MethodMatcher>(YAML).unwrap();

        assert_eq!(
            parsed,
            MethodMatcher::Exact(vec![Method::GET, Method::HEAD])
        );

        let s = serde_json::to_string(&parsed).unwrap();
        assert_eq!(s, "[\"GET\",\"HEAD\"]");
    }

    #[test]
    pub fn test_parsing_star() {
        const YAML: &str = r#"---
"*"
"#;
        let parsed = serde_yaml::from_str::<MethodMatcher>(YAML).unwrap();
        assert_eq!(parsed, MethodMatcher::All);

        let s = serde_json::to_string(&parsed).unwrap();
        assert_eq!(s, "\"*\"");
    }
}
