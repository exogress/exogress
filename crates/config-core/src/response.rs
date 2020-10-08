use http::{HeaderMap, StatusCode};
use std::hash::{Hash, Hasher};
use url::Url;

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Copy)]
#[serde(deny_unknown_fields)]
pub enum TemplateEngine {
    #[serde(rename = "handlebars")]
    Handlebars,
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Copy)]
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
pub struct CommonResponse {
    #[serde(with = "http_serde::header_map", default)]
    pub headers: HeaderMap,
}

impl Hash for CommonResponse {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for (k, v) in &self.headers {
            k.hash(state);
            v.hash(state);
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash)]
#[serde(deny_unknown_fields)]
pub struct RedirectResponse {
    #[serde(rename = "redirect-type")]
    pub redirect_type: RedirectType,
    pub destination: Url,
    #[serde(flatten)]
    pub common: CommonResponse,
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash)]
#[serde(deny_unknown_fields)]
pub struct ResponseBody {
    #[serde(rename = "content-type")]
    pub content_type: String,
    pub content: String,
    pub engine: Option<TemplateEngine>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash)]
#[serde(deny_unknown_fields)]
pub struct RawResponse {
    #[serde(
        rename = "status-code",
        with = "http_serde::status_code",
        default = "default_status_code"
    )]
    pub status_code: StatusCode,

    #[serde(default)]
    pub body: Vec<ResponseBody>,

    #[serde(flatten)]
    pub common: CommonResponse,
}

fn default_status_code() -> StatusCode {
    StatusCode::OK
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash)]
#[serde(deny_unknown_fields, tag = "kind")]
pub enum StaticResponse {
    #[serde(rename = "redirect")]
    Redirect(RedirectResponse),

    #[serde(rename = "raw")]
    Raw(RawResponse),
}
