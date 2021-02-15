use crate::config_core::{
    cache::Cache,
    post_processing::PostProcessing,
    rebase::Rebase,
    referenced::{
        google::{bucket::GcsBucket, credentials::GoogleCredentials},
        Container,
    },
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, schemars::JsonSchema)]
pub struct GcsBucketAccess {
    pub bucket: Container<GcsBucket>,
    pub credentials: Container<GoogleCredentials>,

    #[serde(flatten)]
    pub rebase: Rebase,

    #[serde(default)]
    pub cache: Cache,

    #[serde(rename = "post-processing", default)]
    pub post_processing: PostProcessing,
}
