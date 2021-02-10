use serde::{Deserialize, Serialize};

use crate::{
    config_core::{cache::Cache, post_processing::PostProcessing, rebase::Rebase},
    entities::Upstream,
};

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

    #[serde(
        default = "default_websockets",
        skip_serializing_if = "is_default_websockets"
    )]
    pub websockets: bool,
}

fn default_websockets() -> bool {
    true
}

fn is_default_websockets(val: &bool) -> bool {
    val == &default_websockets()
}
