use crate::config_core::parametrized::acl::Acl;
use crate::config_core::parametrized::aws::bucket::S3Bucket;
use crate::config_core::parametrized::aws::credentials::AwsCredentials;
use crate::config_core::parametrized::google::bucket::GcsBucket;
use crate::config_core::parametrized::google::credentials::GoogleCredentials;
pub use container::Container;
use core::convert::TryFrom;
use core::fmt;
use core::fmt::Formatter;
use core::str::FromStr;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

pub mod acl;
pub mod aws;
pub mod google;

mod container;

pub use container::Error;

pub trait ParameterOrConfigValue:
    DeserializeOwned
    + Serialize
    + core::fmt::Debug
    + Clone
    + Eq
    + PartialEq
    + std::hash::Hash
    + TryFrom<Parameter>
{
    fn schema() -> ParameterSchema;
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(deny_unknown_fields, tag = "schema", content = "body")]
pub enum Parameter {
    #[serde(rename = "aws-credentials")]
    AwsCredentials(AwsCredentials),
    #[serde(rename = "s3-bucket")]
    S3Bucket(S3Bucket),

    #[serde(rename = "google-credentials")]
    GoogleCredentials(GoogleCredentials),
    #[serde(rename = "gcs-bucket")]
    GcsBucket(GcsBucket),

    #[serde(rename = "acl")]
    Acl(Acl),
    // #[serde(rename = "string")]
    // String(SmolStr),
}

impl Parameter {
    pub fn schema(&self) -> ParameterSchema {
        match self {
            Parameter::AwsCredentials(_) => ParameterSchema::AwsCredentials,
            Parameter::S3Bucket(_) => ParameterSchema::S3Bucket,
            Parameter::GoogleCredentials(_) => ParameterSchema::GoogleCredentials,
            Parameter::GcsBucket(_) => ParameterSchema::GcsBucket,
            Parameter::Acl(_) => ParameterSchema::Acl,
            // Parameter::String(_) => ParameterSchema::S
        }
    }
    pub fn to_inner_yaml(&self) -> String {
        match self {
            Parameter::AwsCredentials(inner) => serde_yaml::to_string(&inner).unwrap(),
            Parameter::S3Bucket(inner) => serde_yaml::to_string(&inner).unwrap(),
            Parameter::GoogleCredentials(inner) => serde_yaml::to_string(&inner).unwrap(),
            Parameter::GcsBucket(inner) => serde_yaml::to_string(&inner).unwrap(),
            Parameter::Acl(inner) => serde_yaml::to_string(&inner).unwrap(),
            // Parameter::String(inner) => serde_yaml::to_string(&inner).unwrap(),
        }
    }

    pub fn to_inner_json(&self) -> String {
        match self {
            Parameter::AwsCredentials(inner) => serde_json::to_string_pretty(&inner).unwrap(),
            Parameter::S3Bucket(inner) => serde_json::to_string_pretty(&inner).unwrap(),
            Parameter::GoogleCredentials(inner) => serde_json::to_string_pretty(&inner).unwrap(),
            Parameter::GcsBucket(inner) => serde_json::to_string_pretty(&inner).unwrap(),
            Parameter::Acl(inner) => serde_json::to_string_pretty(&inner).unwrap(),
            // Parameter::String(inner) => serde_json::to_string_pretty(&inner).unwrap(),
        }
    }
}

pub const ALL_PARAMETER_SCHEMAS: [ParameterSchema; 5] = [
    ParameterSchema::AwsCredentials,
    ParameterSchema::S3Bucket,

    ParameterSchema::GoogleCredentials,
    ParameterSchema::GcsBucket,

    ParameterSchema::Acl,
];

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, Copy)]
#[serde(deny_unknown_fields, tag = "kind")]
pub enum ParameterSchema {
    #[serde(rename = "aws-credentials")]
    AwsCredentials,
    #[serde(rename = "s3-bucket")]
    S3Bucket,

    #[serde(rename = "google-credentials")]
    GoogleCredentials,
    #[serde(rename = "gcs-bucket")]
    GcsBucket,

    #[serde(rename = "acl")]
    Acl,
}

impl ParameterSchema {
    pub fn help(&self) -> String {
        match self {
            Self::AwsCredentials => {
                let sample = AwsCredentials {
                    access_key_id: "AKIA2Q9SUGVTPX21NLLN".into(),
                    secret_access_key: "<SECRET_ACCESS_KEY>".into()
                };
                serde_yaml::to_string(&sample).unwrap()
            }
            Self::S3Bucket => {
                let sample = S3Bucket {
                    name: "my-s3-bucket".into(),
                    region: aws::bucket::S3Region::UsEast1
                };
                serde_yaml::to_string(&sample).unwrap()
            },
            Self::GoogleCredentials => {
                let sample = GoogleCredentials {
                    json: "<GCS_JSON>".into(),
                };
                serde_yaml::to_string(&sample).unwrap()
            },
            Self::GcsBucket => {
                let sample = GcsBucket {
                    name: "my-gcs-bucket".into(),
                };
                serde_yaml::to_string(&sample).unwrap()
            },
            Self::Acl => {
                let sample = Acl(vec![
                    acl::AclEntry::Allow { identity: "user@example.com".into() },
                    acl::AclEntry::Deny { identity: "*@example.com".into() },
                    acl::AclEntry::Allow { identity: "*".into() },
                ]);
                serde_yaml::to_string(&sample).unwrap()
            },
        }
    }

}

impl fmt::Display for ParameterSchema {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let s = match self {
            ParameterSchema::AwsCredentials => "aws-credentials",
            ParameterSchema::S3Bucket => "s3-bucket",
            ParameterSchema::GoogleCredentials => "google-credentials",
            ParameterSchema::GcsBucket => "gcs-bucket",
            ParameterSchema::Acl => "acl",
        };

        write!(f, "{}", s)
    }
}

impl FromStr for ParameterSchema {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "aws-credentials" => Ok(ParameterSchema::AwsCredentials),
            "s3-bucket" => Ok(ParameterSchema::S3Bucket),
            "google-credentials" => Ok(ParameterSchema::GoogleCredentials),
            "gcs-bucket" => Ok(ParameterSchema::GcsBucket),
            "acl" => Ok(ParameterSchema::Acl),
            _ => Err(()),
        }
    }
}

impl TryFrom<(ParameterSchema, String)> for Parameter {
    type Error = anyhow::Error;

    fn try_from((schema, value_str): (ParameterSchema, String)) -> Result<Self, Self::Error> {
        match (schema, value_str) {
            (ParameterSchema::AwsCredentials, s) => {
                Ok(Parameter::AwsCredentials(serde_yaml::from_str(s.as_str())?))
            }
            (ParameterSchema::S3Bucket, s) => {
                Ok(Parameter::S3Bucket(serde_yaml::from_str(s.as_str())?))
            }
            (ParameterSchema::GoogleCredentials, s) => Ok(Parameter::GoogleCredentials(
                serde_yaml::from_str(s.as_str())?,
            )),
            (ParameterSchema::GcsBucket, s) => {
                Ok(Parameter::GcsBucket(serde_yaml::from_str(s.as_str())?))
            }
            (ParameterSchema::Acl, s) => Ok(Parameter::Acl(serde_yaml::from_str(s.as_str())?)),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_param() {
        let var = r#"
---
schema: aws-credentials
body:
    access_key_id: asdfasdfasdf
    secret_access_key: asdfasdfasdfasdf 
"#;
        serde_yaml::from_str::<Parameter>(var).unwrap();
    }
}
