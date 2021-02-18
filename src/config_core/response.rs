use crate::config_core::{
    is_default,
    redirect::RedirectTo,
    referenced::mime_types::MimeType,
    rule::{HeaderMapWrapper, ModifyQuery},
    StatusCode,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use std::hash::Hash;

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Copy, Eq, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub enum TemplateEngine {
    #[serde(rename = "handlebars")]
    Handlebars,
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Copy, Eq, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub enum RedirectType {
    #[serde(rename = "moved-permanently")]
    MovedPermanently,

    #[serde(rename = "permanent-redirect")]
    PermanentRedirect,

    #[serde(rename = "found")]
    Found,

    #[serde(rename = "see-other")]
    SeeOther,

    #[serde(rename = "temporary-redirect")]
    TemporaryRedirect,

    #[serde(rename = "multiple-choices")]
    MultipleChoices,

    #[serde(rename = "not-modified")]
    NotModified,
}

impl RedirectType {
    pub fn status_code(&self) -> StatusCode {
        let code = match self {
            RedirectType::MovedPermanently => http::StatusCode::MOVED_PERMANENTLY,
            RedirectType::PermanentRedirect => http::StatusCode::PERMANENT_REDIRECT,
            RedirectType::Found => http::StatusCode::FOUND,
            RedirectType::SeeOther => http::StatusCode::SEE_OTHER,
            RedirectType::TemporaryRedirect => http::StatusCode::TEMPORARY_REDIRECT,
            RedirectType::MultipleChoices => http::StatusCode::MULTIPLE_CHOICES,
            RedirectType::NotModified => http::StatusCode::NOT_MODIFIED,
        };
        StatusCode(code)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RedirectResponse {
    #[serde(rename = "redirect-type")]
    pub redirect_type: RedirectType,
    pub destination: RedirectTo,

    #[serde(default, rename = "query-params", skip_serializing_if = "is_default")]
    pub query_params: ModifyQuery,

    #[serde(default, skip_serializing_if = "is_default")]
    pub headers: HeaderMapWrapper,
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResponseBody {
    #[serde(rename = "content-type")]
    pub content_type: MimeType,
    pub content: SmolStr,
    pub engine: Option<TemplateEngine>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawResponse {
    #[serde(
        rename = "status-code",
        default = "default_status_code",
        skip_serializing_if = "is_default_status_code"
    )]
    pub status_code: StatusCode,

    #[serde(
        rename = "fallback-accept",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub fallback_accept: Option<MimeType>,

    pub body: Vec<ResponseBody>,

    #[serde(default, skip_serializing_if = "is_default")]
    pub headers: HeaderMapWrapper,
}

fn default_status_code() -> StatusCode {
    StatusCode(http::StatusCode::OK)
}

fn is_default_status_code(code: &StatusCode) -> bool {
    code == &StatusCode(http::StatusCode::OK)
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq, JsonSchema)]
#[serde(tag = "kind")]
pub enum StaticResponse {
    #[serde(rename = "redirect")]
    Redirect(RedirectResponse),

    #[serde(rename = "raw")]
    Raw(RawResponse),
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn test_schema() {
        serde_json::to_string_pretty(&schemars::schema_for!(ResponseBody)).unwrap();
        serde_json::to_string_pretty(&schemars::schema_for!(RawResponse)).unwrap();
    }

    #[test]
    fn test_redirect_url_parsing() {
        serde_yaml::from_str::<RedirectResponse>(
            r#"
---
redirect-type: moved-permanently
destination: ["ru.html"]
"#,
        )
        .unwrap();
        serde_yaml::from_str::<RedirectResponse>(
            r#"
---
redirect-type: moved-permanently
destination: ["https://google.com", "a", "b"]
"#,
        )
        .unwrap();
    }
}
