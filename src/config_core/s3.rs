use core::fmt::{self, Formatter};
use serde::de::Visitor;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use smol_str::SmolStr;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct S3Region(s3::Region);

impl Hash for S3Region {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.0.to_string().as_bytes())
    }
}

impl Serialize for S3Region {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.0.to_string().as_str())
    }
}

struct S3RegionVisitor;

impl<'de> Visitor<'de> for S3RegionVisitor {
    type Value = S3Region;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "s3 region name")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value
            .parse()
            .map_err(|e| de::Error::custom(format!("unknown S3 region: {}", e)))
            .map(S3Region)
    }
}

impl<'de> Deserialize<'de> for S3Region {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(S3RegionVisitor)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(deny_unknown_fields)]
pub struct S3Bucket {
    pub bucket: SmolStr,
    pub region: S3Region,
    pub secret_key: Option<SmolStr>,
    pub access_key: Option<SmolStr>,
}

impl From<S3Region> for s3::Region {
    fn from(region: S3Region) -> Self {
        region.0
    }
}
