use schemars::{gen::SchemaGenerator, schema::Schema, JsonSchema};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

#[derive(Debug, Hash, Eq, Serialize, Deserialize, PartialEq, Clone)]
#[serde(transparent)]
pub struct ConfigVersion(pub Version);

impl JsonSchema for ConfigVersion {
    fn schema_name() -> String {
        unimplemented!()
    }

    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        unimplemented!()
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
