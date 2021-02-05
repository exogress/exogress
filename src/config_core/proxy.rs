use serde::{Deserialize, Serialize};

use crate::config_core::cache::Cache;
use crate::config_core::post_processing::PostProcessing;
use crate::config_core::rebase::Rebase;
use crate::entities::Upstream;

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(deny_unknown_fields)]
pub struct Proxy {
    pub upstream: Upstream,

    #[serde(flatten, default)]
    pub rebase: Rebase,

    #[serde(default)]
    pub cache: Cache,

    #[serde(rename = "post-processing", default)]
    pub post_processing: PostProcessing,
}
