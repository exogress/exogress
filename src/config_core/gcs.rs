use crate::config_core::{
    cache::Cache,
    parametrized::{
        google::{bucket::GcsBucket, credentials::GoogleCredentials},
        Container,
    },
    post_processing::PostProcessing,
    rebase::Rebase,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(deny_unknown_fields)]
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
