use crate::config_core::cache::Cache;
use crate::config_core::parametrized::google::bucket::GcsBucket;
use crate::config_core::parametrized::google::credentials::GoogleCredentials;
use crate::config_core::parametrized::Container;
use crate::config_core::post_processing::PostProcessing;
use crate::config_core::rebase::Rebase;
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
