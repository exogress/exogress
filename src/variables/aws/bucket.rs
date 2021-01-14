use core::fmt::{self, Formatter};
use serde::de::Visitor;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use smol_str::SmolStr;
use std::hash::Hash;
use std::str::FromStr;
use url::Url;

// https://github.com/durch/rust-s3/blob/45dd3f25a4047186e414e47fcedb4f83e492368e/aws-region/src/region.rs

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum S3Region {
    /// us-east-1
    UsEast1,
    /// us-east-2
    UsEast2,
    /// us-west-1
    UsWest1,
    /// us-west-2
    UsWest2,
    /// ca-central-1
    CaCentral1,
    /// ap-south-1
    ApSouth1,
    /// ap-northeast-1
    ApNortheast1,
    /// ap-northeast-2
    ApNortheast2,
    /// ap-northeast-3
    ApNortheast3,
    /// ap-southeast-1
    ApSoutheast1,
    /// ap-southeast-2
    ApSoutheast2,
    /// cn-north-1
    CnNorth1,
    /// cn-northwest-1
    CnNorthwest1,
    /// eu-north-1
    EuNorth1,
    /// eu-central-1
    EuCentral1,
    /// eu-west-1
    EuWest1,
    /// eu-west-2
    EuWest2,
    /// eu-west-3
    EuWest3,
    /// me-south-1
    MeSouth1,
    /// sa-east-1
    SaEast1,
    /// Digital Ocean nyc3
    DoNyc3,
    /// Digital Ocean ams3
    DoAms3,
    /// Digital Ocean sgp1
    DoSgp1,
    /// Yandex Object Storage
    Yandex,
    /// Wasabi us-east-1
    WaUsEast1,
    /// Wasabi us-east-2
    WaUsEast2,
    /// Wasabi us-west-1
    WaUsWest1,
    /// Wasabi eu-central-1
    WaEuCentral1,
    /// Custom region
    Custom { region: String, endpoint: String },
}

impl fmt::Display for S3Region {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::S3Region::*;
        match *self {
            UsEast1 => write!(f, "us-east-1"),
            UsEast2 => write!(f, "us-east-2"),
            UsWest1 => write!(f, "us-west-1"),
            UsWest2 => write!(f, "us-west-2"),
            CaCentral1 => write!(f, "ca-central-1"),
            ApSouth1 => write!(f, "ap-south-1"),
            ApNortheast1 => write!(f, "ap-northeast-1"),
            ApNortheast2 => write!(f, "ap-northeast-2"),
            ApNortheast3 => write!(f, "ap-northeast-3"),
            ApSoutheast1 => write!(f, "ap-southeast-1"),
            ApSoutheast2 => write!(f, "ap-southeast-2"),
            CnNorth1 => write!(f, "cn-north-1"),
            CnNorthwest1 => write!(f, "cn-northwest-1"),
            EuNorth1 => write!(f, "eu-north-1"),
            EuCentral1 => write!(f, "eu-central-1"),
            EuWest1 => write!(f, "eu-west-1"),
            EuWest2 => write!(f, "eu-west-2"),
            EuWest3 => write!(f, "eu-west-3"),
            SaEast1 => write!(f, "sa-east-1"),
            MeSouth1 => write!(f, "me-south-1"),
            DoNyc3 => write!(f, "nyc3"),
            DoAms3 => write!(f, "ams3"),
            DoSgp1 => write!(f, "sgp1"),
            Yandex => write!(f, "ru-central1"),
            WaUsEast1 => write!(f, "us-east-1"),
            WaUsEast2 => write!(f, "us-east-2"),
            WaUsWest1 => write!(f, "us-west-1"),
            WaEuCentral1 => write!(f, "eu-central-1"),
            Custom { ref region, .. } => write!(f, "{}", region.to_string()),
        }
    }
}

impl FromStr for S3Region {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, ()> {
        use self::S3Region::*;
        match s {
            "us-east-1" => Ok(UsEast1),
            "us-east-2" => Ok(UsEast2),
            "us-west-1" => Ok(UsWest1),
            "us-west-2" => Ok(UsWest2),
            "ca-central-1" => Ok(CaCentral1),
            "ap-south-1" => Ok(ApSouth1),
            "ap-northeast-1" => Ok(ApNortheast1),
            "ap-northeast-2" => Ok(ApNortheast2),
            "ap-northeast-3" => Ok(ApNortheast3),
            "ap-southeast-1" => Ok(ApSoutheast1),
            "ap-southeast-2" => Ok(ApSoutheast2),
            "cn-north-1" => Ok(CnNorth1),
            "cn-northwest-1" => Ok(CnNorthwest1),
            "eu-north-1" => Ok(EuNorth1),
            "eu-central-1" => Ok(EuCentral1),
            "eu-west-1" => Ok(EuWest1),
            "eu-west-2" => Ok(EuWest2),
            "eu-west-3" => Ok(EuWest3),
            "sa-east-1" => Ok(SaEast1),
            "me-south-1" => Ok(MeSouth1),
            "nyc3" => Ok(DoNyc3),
            "ams3" => Ok(DoAms3),
            "sgp1" => Ok(DoSgp1),
            "yandex" => Ok(Yandex),
            "ru-central1" => Ok(Yandex),
            "wa-us-east-1" => Ok(WaUsEast1),
            "wa-us-east-2" => Ok(WaUsEast2),
            "wa-us-west-1" => Ok(WaUsWest1),
            "wa-eu-central-1" => Ok(WaEuCentral1),
            x => Ok(Custom {
                region: x.to_string(),
                endpoint: x.to_string(),
            }),
        }
    }
}

impl S3Region {
    pub fn endpoint(&self) -> Url {
        use self::S3Region::*;
        match *self {
            // Surprisingly, us-east-1 does not have a
            // s3-us-east-1.amazonaws.com DNS record
            UsEast1 => Url::from_str("https://s3.amazonaws.com").unwrap(),
            UsEast2 => Url::from_str("https://s3-us-east-2.amazonaws.com").unwrap(),
            UsWest1 => Url::from_str("https://s3-us-west-1.amazonaws.com").unwrap(),
            UsWest2 => Url::from_str("https://s3-us-west-2.amazonaws.com").unwrap(),
            CaCentral1 => Url::from_str("https://s3-ca-central-1.amazonaws.com").unwrap(),
            ApSouth1 => Url::from_str("https://s3-ap-south-1.amazonaws.com").unwrap(),
            ApNortheast1 => Url::from_str("https://s3-ap-northeast-1.amazonaws.com").unwrap(),
            ApNortheast2 => Url::from_str("https://s3-ap-northeast-2.amazonaws.com").unwrap(),
            ApNortheast3 => Url::from_str("https://s3-ap-northeast-3.amazonaws.com").unwrap(),
            ApSoutheast1 => Url::from_str("https://s3-ap-southeast-1.amazonaws.com").unwrap(),
            ApSoutheast2 => Url::from_str("https://s3-ap-southeast-2.amazonaws.com").unwrap(),
            CnNorth1 => Url::from_str("https://s3.cn-north-1.amazonaws.com.cn").unwrap(),
            CnNorthwest1 => Url::from_str("https://s3.cn-northwest-1.amazonaws.com.cn").unwrap(),
            EuNorth1 => Url::from_str("https://s3-eu-north-1.amazonaws.com").unwrap(),
            EuCentral1 => Url::from_str("https://s3-eu-central-1.amazonaws.com").unwrap(),
            EuWest1 => Url::from_str("https://s3-eu-west-1.amazonaws.com").unwrap(),
            EuWest2 => Url::from_str("https://s3-eu-west-2.amazonaws.com").unwrap(),
            EuWest3 => Url::from_str("https://s3-eu-west-3.amazonaws.com").unwrap(),
            SaEast1 => Url::from_str("https://s3-sa-east-1.amazonaws.com").unwrap(),
            MeSouth1 => Url::from_str("https://s3-me-south-1.amazonaws.com").unwrap(),
            DoNyc3 => Url::from_str("https://nyc3.digitaloceanspaces.com").unwrap(),
            DoAms3 => Url::from_str("https://ams3.digitaloceanspaces.com").unwrap(),
            DoSgp1 => Url::from_str("https://sgp1.digitaloceanspaces.com").unwrap(),
            Yandex => Url::from_str("https://storage.yandexcloud.net").unwrap(),
            WaUsEast1 => Url::from_str("https://s3.us-east-1.wasabisys.com").unwrap(),
            WaUsEast2 => Url::from_str("https://s3.us-east-2.wasabisys.com").unwrap(),
            WaUsWest1 => Url::from_str("https://s3.us-west-1.wasabisys.com").unwrap(),
            WaEuCentral1 => Url::from_str("https://s3.eu-central-1.wasabisys.com").unwrap(),
            Custom { ref endpoint, .. } => endpoint.parse().unwrap(),
        }
    }
}

impl Serialize for S3Region {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_str())
    }
}

struct S3RegionVisitor;

impl<'de> Visitor<'de> for S3RegionVisitor {
    type Value = S3Region;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "s3 region name or url")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value
            .parse()
            .map_err(|_e| de::Error::custom("unknown S3 region"))
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
}
