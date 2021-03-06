use serde::{Deserialize, Serialize};

use crate::{
    config_core::{cache::Cache, post_processing::PostProcessing, rebase::Rebase},
    entities::Upstream,
};
use schemars::JsonSchema;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Hash, JsonSchema)]
// #[schemars(deny_unknown_fields)]
pub struct Proxy {
    pub upstream: Upstream,

    #[serde(flatten, default)]
    pub rebase: Rebase,

    #[serde(default)]
    pub cache: Cache,

    #[serde(rename = "post-processing", default)]
    pub post_processing: PostProcessing,

    #[serde(default = "default_websockets")]
    pub websockets: bool,
}

fn default_websockets() -> bool {
    true
}
