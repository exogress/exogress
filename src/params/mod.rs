use crate::params::acl::Acl;
use crate::params::aws::bucket::S3Bucket;
use crate::params::aws::credentials::AwsCredentials;
use crate::params::google::bucket::GcsBucket;
use crate::params::google::credentials::GoogleCredentials;
use core::convert::TryFrom;
use core::str::FromStr;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

pub mod acl;
pub mod aws;
pub mod google;

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

    #[serde(rename = "string")]
    String(SmolStr),
}

impl Parameter {
    pub fn to_inner_yaml(&self) -> String {
        match self {
            Parameter::AwsCredentials(inner) => serde_yaml::to_string(&inner).unwrap(),
            Parameter::S3Bucket(inner) => serde_yaml::to_string(&inner).unwrap(),
            Parameter::GoogleCredentials(inner) => serde_yaml::to_string(&inner).unwrap(),
            Parameter::GcsBucket(inner) => serde_yaml::to_string(&inner).unwrap(),
            Parameter::Acl(inner) => serde_yaml::to_string(&inner).unwrap(),
            Parameter::String(inner) => serde_yaml::to_string(&inner).unwrap(),
        }
    }

    pub fn to_inner_json(&self) -> String {
        match self {
            Parameter::AwsCredentials(inner) => serde_json::to_string_pretty(&inner).unwrap(),
            Parameter::S3Bucket(inner) => serde_json::to_string_pretty(&inner).unwrap(),
            Parameter::GoogleCredentials(inner) => serde_json::to_string_pretty(&inner).unwrap(),
            Parameter::GcsBucket(inner) => serde_json::to_string_pretty(&inner).unwrap(),
            Parameter::Acl(inner) => serde_json::to_string_pretty(&inner).unwrap(),
            Parameter::String(inner) => serde_json::to_string_pretty(&inner).unwrap(),
        }
    }

    pub fn aws_credentials(&self) -> Option<&AwsCredentials> {
        match self {
            Parameter::AwsCredentials(creds) => Some(creds),
            _ => None,
        }
    }

    pub fn s3_bucket(&self) -> Option<&S3Bucket> {
        match self {
            Parameter::S3Bucket(s3) => Some(s3),
            _ => None,
        }
    }

    pub fn google_credentials(&self) -> Option<&GoogleCredentials> {
        match self {
            Parameter::GoogleCredentials(creds) => Some(creds),
            _ => None,
        }
    }

    pub fn gcs_bucket(&self) -> Option<&GcsBucket> {
        match self {
            Parameter::GcsBucket(gcs) => Some(gcs),
            _ => None,
        }
    }

    pub fn acl(&self) -> Option<&Acl> {
        match self {
            Parameter::Acl(acl) => Some(acl),
            _ => None,
        }
    }

    pub fn string(&self) -> Option<&SmolStr> {
        match self {
            Parameter::String(s) => Some(s),
            _ => None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
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

    #[serde(rename = "string")]
    String,
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
            "string" => Ok(ParameterSchema::String),
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
            (ParameterSchema::String, s) => {
                Ok(Parameter::String(serde_yaml::from_str(s.as_str())?))
            }
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
