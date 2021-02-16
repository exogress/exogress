use crate::config_core::{
    cache::Cache, post_processing::PostProcessing, rebase::Rebase, StatusCode,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema)]
pub enum Wildcard {
    #[serde(rename = "_")]
    Any,
    #[serde(rename = "5xx")]
    ServerErrors,
    #[serde(rename = "4xx")]
    ClientErrors,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema)]
#[serde(untagged)]
pub enum Error {
    StatusCode(StatusCode),
    Placeholder(Wildcard),
}

impl From<http::StatusCode> for Error {
    fn from(s: http::StatusCode) -> Self {
        Error::StatusCode(StatusCode(s))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema)]
pub struct StaticDir {
    pub dir: PathBuf,

    #[serde(flatten)]
    pub rebase: Rebase,

    #[serde(default)]
    pub cache: Cache,

    #[serde(rename = "post-processing", default)]
    pub post_processing: PostProcessing,
}

#[cfg(test)]
mod test {

    //     #[test]
    //     pub fn test_deserialize() {
    //         assert_eq!(
    //             StaticDir {
    //                 path: "./web".parse().unwrap(),
    //                 cache_policy: Some("immutable".parse().unwrap()),
    //                 errors: vec![
    //                     (
    //                         http::StatusCode::NOT_FOUND.into(),
    //                         "errors/404.html".parse().unwrap()
    //                     ),
    //                     (
    //                         Error::Placeholder(Wildcard::ServerErrors),
    //                         "errors/500.html".parse().unwrap()
    //                     ),
    //                     (
    //                         Error::Placeholder(Wildcard::Any),
    //                         "errors/_.html".parse().unwrap()
    //                     ),
    //                 ],
    //             },
    //             serde_yaml::from_str(
    //                 r#"---
    // path: "./web"
    // cache_policy: immutable
    // errors:
    //   - [404, "errors/404.html"]
    //   - [5xx, "errors/500.html"]
    //   - [_, "errors/_.html"]
    // "#
    //             )
    //             .unwrap()
    //         );
    //     }
}
