use crate::entities::ProfileName;
pub use auth::{Auth, AuthDefinition, AuthProvider};
pub use catch::{
    CatchAction, CatchMatcher, CatchMatcherParseError, Exception, ExceptionParseError, RescueItem,
};
pub use client_config::{
    ClientConfig, ClientConfigRevision, ClientHandler, ClientHandlerVariant, ClientMount,
};
pub use config::{default_rules, Config};
use lazy_static::lazy_static;
pub use methods::MethodMatcher;
pub use pass_through::PassThrough;
pub use path::{MatchPathSegment, MatchPathSingleSegment, MatchingPath};
pub use path_segment::UrlPathSegmentOrQueryPart;
pub use project_config::{ProjectConfig, ProjectHandler, ProjectHandlerVariant};
pub use proxy::Proxy;
pub use rebase::Rebase;
pub use response::{
    HttpHeaders, RawResponse, RedirectResponse, ResponseBody, StaticResponse, TemplateEngine,
};
pub use rule::{
    Action, Filter, ModifyHeaders, OnResponse, RequestModifications, ResponseModifications, Rule,
    TrailingSlashFilterRule,
};
pub use static_dir::StaticDir;
pub use status_code::{StatusCode, StatusCodeRange};
pub use upstream::{Probe, UpstreamDefinition, UpstreamSocketAddr};
pub use version::ConfigVersion;

mod application_firewall;
mod auth;
mod catch;
mod client_config;
mod config;
mod gcs;
mod methods;
pub mod parametrized;
mod pass_through;
mod path;
mod path_segment;
mod project_config;
mod proxy;
mod rebase;
mod redirect;
mod response;
mod rewrite;
mod rule;
mod s3;
mod serde_as;
mod static_dir;
mod status_code;
mod upstream;
mod version;

pub const DEFAULT_CONFIG_FILE: &str = "Exofile";

lazy_static! {
    pub static ref CURRENT_VERSION: ConfigVersion = ConfigVersion("1.0.0-pre.1".parse().unwrap());
}

pub fn is_profile_active(
    profiles: &Option<Vec<ProfileName>>,
    active_profile: &Option<ProfileName>,
) -> bool {
    match profiles {
        None => true,
        Some(allowed_profiles) => match active_profile {
            None => false,
            Some(profile) => allowed_profiles.iter().any(|allowed| allowed == profile),
        },
    }
}
