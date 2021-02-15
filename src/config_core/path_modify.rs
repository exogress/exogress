use serde::{Deserialize, Serialize};

use smol_str::SmolStr;
use std::str::FromStr;

#[derive(Serialize, Deserialize, Debug, Hash, Clone, Eq, PartialEq, schemars::JsonSchema)]
#[serde(transparent)]
pub struct PathSegmentsModify(pub SmolStr);

impl AsRef<str> for PathSegmentsModify {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl PathSegmentsModify {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl FromStr for PathSegmentsModify {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(PathSegmentsModify(s.into()))
    }
}
