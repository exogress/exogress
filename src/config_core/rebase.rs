use crate::config_core::UrlPathSegmentOrQueryPart;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, Default)]
pub struct Rebase {
    #[serde(default, rename = "base-path")]
    pub base_path: Vec<UrlPathSegmentOrQueryPart>,

    #[serde(default, rename = "replace-base-path")]
    pub replace_base_path: Vec<UrlPathSegmentOrQueryPart>,
}
