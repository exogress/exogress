use crate::config_core::UrlPathSegment;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, Default, JsonSchema)]
pub struct Rebase {
    #[serde(default, rename = "base-path", skip_serializing_if = "Vec::is_empty")]
    pub base_path: Vec<UrlPathSegment>,

    #[serde(
        default,
        rename = "replace-base-path",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub replace_base_path: Vec<UrlPathSegment>,
}
