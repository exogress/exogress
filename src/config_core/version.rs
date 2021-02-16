use crate::config_core::CURRENT_VERSION;
use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

#[derive(Debug, Hash, Eq, Serialize, Deserialize, PartialEq, Clone, JsonSchema)]
#[serde(transparent)]
pub struct ConfigVersion(pub Version);

impl ConfigVersion {
    pub fn minor_base_version(&self) -> String {
        format!(
            "{}.{}",
            (*CURRENT_VERSION).0.major,
            (*CURRENT_VERSION).0.minor
        )
    }

    pub fn major_base_version(&self) -> String {
        (*CURRENT_VERSION).0.major.to_string()
    }
}

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
