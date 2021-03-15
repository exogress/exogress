use crate::config_core::referenced::{
    acl::Acl,
    aws::{bucket::S3Bucket, credentials::AwsCredentials},
    google::{bucket::GcsBucket, credentials::GoogleCredentials},
    mime_types::MimeTypes,
};
pub use container::Container;
use core::{convert::TryFrom, fmt, fmt::Formatter, str::FromStr};
use schemars::JsonSchema;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub mod acl;
pub mod aws;
pub mod google;
pub mod mime_types;
pub mod static_response;

mod container;

use crate::config_core::{
    referenced::mime_types::MimeType, RawResponse, ResponseBody, StaticResponse, StatusCode,
};
pub use container::Error;

pub trait ReferencedConfigValue:
    DeserializeOwned
    + Serialize
    + core::fmt::Debug
    + Clone
    + Eq
    + PartialEq
    + std::hash::Hash
    + TryFrom<Parameter>
    + JsonSchema
{
    fn schema() -> ParameterSchema;
}

impl ReferencedConfigValue for () {
    fn schema() -> ParameterSchema {
        unreachable!()
    }
}

impl TryFrom<Parameter> for () {
    type Error = ();

    fn try_from(_value: Parameter) -> Result<Self, Self::Error> {
        unimplemented!()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(tag = "schema", content = "body")]
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

    #[serde(rename = "mime-types")]
    MimeTypes(MimeTypes),

    #[serde(rename = "static-response")]
    StaticResponse(Box<StaticResponse>),
}

impl Parameter {
    pub fn schema(&self) -> ParameterSchema {
        match self {
            Parameter::AwsCredentials(_) => ParameterSchema::AwsCredentials,
            Parameter::S3Bucket(_) => ParameterSchema::S3Bucket,
            Parameter::GoogleCredentials(_) => ParameterSchema::GoogleCredentials,
            Parameter::GcsBucket(_) => ParameterSchema::GcsBucket,
            Parameter::Acl(_) => ParameterSchema::Acl,
            Parameter::MimeTypes(_) => ParameterSchema::MimeTypes,
            Parameter::StaticResponse(_) => ParameterSchema::StaticResponse,
        }
    }
    pub fn to_inner_yaml(&self) -> String {
        match self {
            Parameter::AwsCredentials(inner) => serde_yaml::to_string(&inner).unwrap(),
            Parameter::S3Bucket(inner) => serde_yaml::to_string(&inner).unwrap(),
            Parameter::GoogleCredentials(inner) => serde_yaml::to_string(&inner).unwrap(),
            Parameter::GcsBucket(inner) => serde_yaml::to_string(&inner).unwrap(),
            Parameter::Acl(inner) => serde_yaml::to_string(&inner).unwrap(),
            Parameter::MimeTypes(inner) => serde_yaml::to_string(&inner).unwrap(),
            Parameter::StaticResponse(resp) => serde_yaml::to_string(&resp).unwrap(),
        }
    }

    pub fn to_inner_json(&self) -> String {
        match self {
            Parameter::AwsCredentials(inner) => serde_json::to_string_pretty(&inner).unwrap(),
            Parameter::S3Bucket(inner) => serde_json::to_string_pretty(&inner).unwrap(),
            Parameter::GoogleCredentials(inner) => serde_json::to_string_pretty(&inner).unwrap(),
            Parameter::GcsBucket(inner) => serde_json::to_string_pretty(&inner).unwrap(),
            Parameter::Acl(inner) => serde_json::to_string_pretty(&inner).unwrap(),
            Parameter::MimeTypes(inner) => serde_json::to_string_pretty(&inner).unwrap(),
            Parameter::StaticResponse(inner) => serde_json::to_string_pretty(&inner).unwrap(),
        }
    }
}

pub const ALL_PARAMETER_SCHEMAS: [ParameterSchema; 7] = [
    ParameterSchema::AwsCredentials,
    ParameterSchema::S3Bucket,
    ParameterSchema::GoogleCredentials,
    ParameterSchema::GcsBucket,
    ParameterSchema::Acl,
    ParameterSchema::MimeTypes,
    ParameterSchema::StaticResponse,
];

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, Copy, JsonSchema)]
// #[schemars(deny_unknown_fields)]
#[serde(tag = "kind")]
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

    #[serde(rename = "mime-types")]
    MimeTypes,

    #[serde(rename = "static-response")]
    StaticResponse,
}

impl ParameterSchema {
    pub fn sample(&self) -> String {
        match self {
            Self::AwsCredentials => {
                let sample = AwsCredentials {
                    access_key_id: "<ACCESS_KEY_ID>".into(),
                    secret_access_key: "<SECRET_ACCESS_KEY>".into(),
                };
                serde_yaml::to_string(&sample).unwrap()
            }
            Self::S3Bucket => {
                let sample = S3Bucket {
                    name: "my-s3-bucket".into(),
                    region: aws::bucket::S3Region::UsEast1,
                };
                serde_yaml::to_string(&sample).unwrap()
            }
            Self::GoogleCredentials => {
                let sample = GoogleCredentials {
                    json: "<GCS_JSON>".into(),
                };
                serde_yaml::to_string(&sample).unwrap()
            }
            Self::GcsBucket => {
                let sample = GcsBucket {
                    name: "my-gcs-bucket".into(),
                };
                serde_yaml::to_string(&sample).unwrap()
            }
            Self::Acl => {
                let sample = Acl(vec![
                    acl::AclEntry::Allow {
                        identity: "user@example.com".into(),
                    },
                    acl::AclEntry::Deny {
                        identity: "*@example.com".into(),
                    },
                    acl::AclEntry::Allow {
                        identity: "*".into(),
                    },
                ]);
                serde_yaml::to_string(&sample).unwrap()
            }
            Self::MimeTypes => {
                let sample: MimeTypes = MimeTypes(vec![MimeType("text/html".parse().unwrap())]);
                serde_yaml::to_string(&sample).unwrap()
            }
            Self::StaticResponse => {
                let sample = StaticResponse::Raw(RawResponse {
                    status_code: StatusCode(http::StatusCode::OK),
                    fallback_accept: None,
                    body: vec![ResponseBody {
                        content_type: mime::TEXT_HTML.into(),
                        content: "<html><body>response</body></html>".into(),
                        engine: None,
                    }],
                    headers: Default::default(),
                });
                serde_yaml::to_string(&sample).unwrap()
            }
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
            ParameterSchema::MimeTypes => "mime-types",
            ParameterSchema::StaticResponse => "static-response",
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
            "mime-types" => Ok(ParameterSchema::MimeTypes),
            "static-response" => Ok(ParameterSchema::StaticResponse),
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
            (ParameterSchema::MimeTypes, s) => {
                Ok(Parameter::MimeTypes(serde_yaml::from_str(s.as_str())?))
            }
            (ParameterSchema::StaticResponse, s) => {
                Ok(Parameter::StaticResponse(serde_yaml::from_str(s.as_str())?))
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
