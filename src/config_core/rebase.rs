use crate::config_core::UrlPathSegment;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, Default, JsonSchema)]
// #[schemars(deny_unknown_fields)]
pub struct Rebase {
    #[serde(default, rename = "base-path")]
    pub base_path: Vec<UrlPathSegment>,

    #[serde(default, rename = "replace-base-path")]
    pub replace_base_path: Vec<UrlPathSegment>,
}
