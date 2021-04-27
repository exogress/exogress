use std::{fmt, str::FromStr};

#[derive(
    Debug, Clone, serde::Serialize, serde::Deserialize, Hash, Eq, PartialEq, Ord, PartialOrd,
)]
#[serde(transparent)]
pub struct LabelValue {
    inner: String,
}

impl FromStr for LabelValue {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(LabelValue { inner: s.into() })
    }
}

impl From<std::string::String> for LabelValue {
    fn from(s: std::string::String) -> Self {
        LabelValue { inner: s }
    }
}

impl fmt::Display for LabelValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.inner)
    }
}
