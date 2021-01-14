use crate::entities::VariableName;
use crate::variables::acl::Acl;
use crate::variables::aws::bucket::S3Bucket;
use crate::variables::aws::credentials::AwsCredentials;
use crate::variables::google::bucket::GcsBucket;
use crate::variables::google::credentials::GoogleCredentials;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

pub mod acl;
pub mod aws;
pub mod google;

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(deny_unknown_fields, tag = "kind")]
pub enum VariableValue {
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
