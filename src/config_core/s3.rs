use crate::config_core::cache::Cache;
use crate::config_core::parametrized::aws::bucket::S3Bucket;
use crate::config_core::parametrized::aws::credentials::AwsCredentials;
use crate::config_core::parametrized::Container;
use crate::config_core::post_processing::PostProcessing;
use crate::config_core::rebase::Rebase;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(deny_unknown_fields)]
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
