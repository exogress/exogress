use http::StatusCode;
use http_serde;
use smartstring::alias::String;

use crate::path::MatchingPath;
use crate::redirect::Redirect;
use crate::rewrite::PathSegmentRewrite;

#[derive(Serialize, Deserialize, Debug, Clone)]
// #[serde(deny_unknown_fields)]
pub struct Mapping {
    path: MatchingPath,
    #[serde(flatten)]
    mapping_type: MappingType,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(deny_unknown_fields, tag = "action")]
pub enum MappingType {
    #[serde(rename = "rewrite")]
    Rewrite(Rewrite),

    #[serde(rename = "redirect")]
    Redirect(Redirect),

    #[serde(rename = "static_response")]
    StaticResponse(StaticResponse),
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Rewrite {
    to: Vec<PathSegmentRewrite>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StaticResponse {
    body: String,

    #[serde(rename = "content-type")]
    content_type: String,

    #[serde(with = "http_serde::status_code")]
    status_code: StatusCode,
}
