use crate::config_core::{
    redirect::RedirectTo,
    referenced::{Parameter, ParameterSchema, ReferencedConfigValue},
};
use http::{HeaderMap, StatusCode};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use smol_str::SmolStr;
use std::{
    convert::TryFrom,
    hash::{Hash, Hasher},
};

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Copy, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub enum TemplateEngine {
    #[serde(rename = "handlebars")]
    Handlebars,
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Copy, Eq, PartialEq)]
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

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct HttpHeaders {
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

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RedirectResponse {
    #[serde(rename = "redirect-type")]
    pub redirect_type: RedirectType,
    pub destination: RedirectTo,
    #[serde(flatten)]
    pub common: HttpHeaders,
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ResponseBody {
    #[serde_as(as = "DisplayFromStr")]
    #[serde(rename = "content-type")]
    pub content_type: mime::Mime,
    pub content: SmolStr,
    pub engine: Option<TemplateEngine>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
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
    pub fallback_accept: Option<SmolStr>,

    pub body: Vec<ResponseBody>,

    #[serde(flatten)]
    pub common: HttpHeaders,
}

fn default_status_code() -> StatusCode {
    StatusCode::OK
}

fn is_default_status_code(code: &StatusCode) -> bool {
    code == &StatusCode::OK
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
#[serde(deny_unknown_fields, tag = "kind")]
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
