use crate::config_core::UrlPathSegmentOrQueryPart;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, Default)]
pub struct Rebase {
    #[serde(default, rename = "base-path", skip_serializing_if = "Vec::is_empty")]
    pub base_path: Vec<UrlPathSegmentOrQueryPart>,

    #[serde(
        default,
        rename = "replace-base-path",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub replace_base_path: Vec<UrlPathSegmentOrQueryPart>,
}
