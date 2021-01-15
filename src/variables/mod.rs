use crate::variables::acl::Acl;
use crate::variables::aws::bucket::S3Bucket;
use crate::variables::aws::credentials::AwsCredentials;
use crate::variables::google::bucket::GcsBucket;
use crate::variables::google::credentials::GoogleCredentials;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use core::str::FromStr;
use core::convert::TryFrom;

pub mod acl;
pub mod aws;
pub mod google;

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(deny_unknown_fields)]
pub enum Variable {
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

impl Variable {
    pub fn to_inner_yaml(&self) -> String {
        match self {
            Variable::AwsCredentials(inner) => serde_yaml::to_string(&inner).unwrap(),
            Variable::S3Bucket(inner) => serde_yaml::to_string(&inner).unwrap(),
            Variable::GoogleCredentials(inner) => serde_yaml::to_string(&inner).unwrap(),
            Variable::GcsBucket(inner) => serde_yaml::to_string(&inner).unwrap(),
            Variable::Acl(inner) => serde_yaml::to_string(&inner).unwrap(),
            Variable::String(inner) => serde_yaml::to_string(&inner).unwrap(),
        }
    }

    pub fn to_inner_json(&self) -> String {
        match self {
            Variable::AwsCredentials(inner) => serde_json::to_string_pretty(&inner).unwrap(),
            Variable::S3Bucket(inner) => serde_json::to_string_pretty(&inner).unwrap(),
            Variable::GoogleCredentials(inner) => serde_json::to_string_pretty(&inner).unwrap(),
            Variable::GcsBucket(inner) => serde_json::to_string_pretty(&inner).unwrap(),
            Variable::Acl(inner) => serde_json::to_string_pretty(&inner).unwrap(),
            Variable::String(inner) => serde_json::to_string_pretty(&inner).unwrap(),
        }


    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(deny_unknown_fields, tag = "kind")]
pub enum VariableSchema {
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

impl FromStr for VariableSchema {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "aws-credentials" => Ok(VariableSchema::AwsCredentials),
            "s3-bucket" => Ok(VariableSchema::S3Bucket),
            "google-credentials" => Ok(VariableSchema::GoogleCredentials),
            "gcs-bucket" => Ok(VariableSchema::GcsBucket),
            "acl" => Ok(VariableSchema::Acl),
            "string" => Ok(VariableSchema::String),
            _ => Err(()),
        }
    }
}

impl TryFrom<(VariableSchema, String)> for Variable {
    type Error = anyhow::Error;

    fn try_from((schema, value_str): (VariableSchema, String)) -> Result<Self, Self::Error> {
        match (schema, value_str) {
            (VariableSchema::AwsCredentials, s) => {
                Ok(Variable::AwsCredentials(
                    serde_yaml::from_str(s.as_str())?
                ))
            }
            (VariableSchema::S3Bucket, s) => {
                Ok(Variable::S3Bucket(
                    serde_yaml::from_str(s.as_str())?
                ))
            }
            (VariableSchema::GoogleCredentials, s) => {
                Ok(Variable::GoogleCredentials(
                    serde_yaml::from_str(s.as_str())?
                ))
            }
            (VariableSchema::GcsBucket, s) => {
                Ok(Variable::GcsBucket(
                    serde_yaml::from_str(s.as_str())?
                ))
            }
            (VariableSchema::Acl, s) => {
                Ok(Variable::Acl(
                    serde_yaml::from_str(s.as_str())?
                ))
            }
            (VariableSchema::String, s) => {
                Ok(Variable::String(
                    serde_yaml::from_str(s.as_str())?
                ))
            }
        }
    }
}
