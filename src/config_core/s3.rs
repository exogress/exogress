use crate::config_core::{
    cache::Cache,
    post_processing::PostProcessing,
    rebase::Rebase,
    referenced::{
        aws::{bucket::S3Bucket, credentials::AwsCredentials},
        Container,
    },
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Hash, JsonSchema)]
// #[schemars(deny_unknown_fields)]
pub struct S3BucketAccess {
    pub bucket: Container<S3Bucket>,
    pub credentials: Option<Container<AwsCredentials>>,

    #[serde(flatten)]
    pub rebase: Rebase,

    #[serde(default)]
    pub cache: Cache,

    #[serde(rename = "post-processing", default)]
    pub post_processing: PostProcessing,
}
