use crate::{
    config_core::redirect::RedirectTo,
    entities::schemars::{gen::SchemaGenerator, schema::Schema},
};
use http::{HeaderMap, StatusCode};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use smol_str::SmolStr;
use std::hash::{Hash, Hasher};

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Copy, Eq, PartialEq, schemars::JsonSchema)]
pub enum TemplateEngine {
    #[serde(rename = "handlebars")]
    Handlebars,
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Copy, Eq, PartialEq, schemars::JsonSchema)]
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
        match self {
            RedirectType::MovedPermanently => StatusCode::MOVED_PERMANENTLY,
            RedirectType::PermanentRedirect => StatusCode::PERMANENT_REDIRECT,
            RedirectType::Found => StatusCode::FOUND,
            RedirectType::SeeOther => StatusCode::SEE_OTHER,
            RedirectType::TemporaryRedirect => StatusCode::TEMPORARY_REDIRECT,
            RedirectType::MultipleChoices => StatusCode::MULTIPLE_CHOICES,
            RedirectType::NotModified => StatusCode::NOT_MODIFIED,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, schemars::JsonSchema)]
pub struct HttpHeaders {
    #[schemars(schema_with = "super::unimplemented_schema")]
    #[serde(
        with = "http_serde::header_map",
        default,
        skip_serializing_if = "HeaderMap::is_empty"
    )]
    pub headers: HeaderMap,
}

impl HttpHeaders {
    pub fn is_default(&self) -> bool {
        self == &Default::default()
    }
}

impl Hash for HttpHeaders {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for (k, v) in &self.headers {
            k.hash(state);
            v.hash(state);
        }
    }
}

impl PartialEq for HttpHeaders {
    fn eq(&self, other: &Self) -> bool {
        let existing: Vec<_> = self.headers.iter().collect();
        let other: Vec<_> = other.headers.iter().collect();
        existing.eq(&other)
    }
}

impl Eq for HttpHeaders {}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq, schemars::JsonSchema)]
pub struct RedirectResponse {
    #[serde(rename = "redirect-type")]
    pub redirect_type: RedirectType,
    pub destination: RedirectTo,
    #[serde(flatten)]
    pub common: HttpHeaders,
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
pub struct ResponseBody {
    #[serde_as(as = "DisplayFromStr")]
    #[serde(rename = "content-type")]
    pub content_type: mime::Mime,
    pub content: SmolStr,
    pub engine: Option<TemplateEngine>,
}

impl schemars::JsonSchema for ResponseBody {
    fn schema_name() -> String {
        unimplemented!()
    }

    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        unimplemented!()
    }
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
pub struct RawResponse {
    #[serde(
        rename = "status-code",
        with = "http_serde::status_code",
        default = "default_status_code",
        skip_serializing_if = "is_default_status_code"
    )]
    pub status_code: StatusCode,

    #[serde(
        rename = "fallback-accept",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub fallback_accept: Option<mime::Mime>,

    pub body: Vec<ResponseBody>,

    #[serde(flatten)]
    pub common: HttpHeaders,
}

impl schemars::JsonSchema for RawResponse {
    fn schema_name() -> String {
        unimplemented!()
    }

    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        unimplemented!()
    }
}

fn default_status_code() -> StatusCode {
    StatusCode::OK
}

fn is_default_status_code(code: &StatusCode) -> bool {
    code == &StatusCode::OK
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq, schemars::JsonSchema)]
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
