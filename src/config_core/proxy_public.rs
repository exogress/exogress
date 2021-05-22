use crate::config_core::{cache::Cache, post_processing::PostProcessing, rebase::Rebase};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

fn default_true() -> bool {
    true
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema)]
// #[schemars(deny_unknown_fields)]
pub struct ProxyPublic {
    pub host: SmolStr,

    #[serde(flatten)]
    pub rebase: Rebase,

    #[serde(default)]
    pub cache: Cache,

    #[serde(rename = "post-processing", default)]
    pub post_processing: PostProcessing,

    #[serde(default = "default_true")]
    pub websockets: bool,
}
