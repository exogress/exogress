use semver::Version;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Hash, Eq, Serialize, Deserialize, PartialEq, Clone)]
#[serde(transparent)]
pub struct ConfigVersion(pub Version);

impl fmt::Display for ConfigVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<Version> for ConfigVersion {
    fn from(version: Version) -> Self {
        ConfigVersion(version)
    }
}

impl FromStr for ConfigVersion {
    type Err = semver::SemVerError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Version::parse(s)?.into())
    }
}
