use crate::entities::ProfileName;
pub use auth::{Auth, AuthDefinition, AuthProvider};
pub use catch::{CatchAction, CatchMatcher, CatchMatcherParseError, RescueItem};
pub use client_config::{
    ClientConfig, ClientConfigRevision, ClientHandler, ClientHandlerVariant, ClientMount,
};
pub use config::{default_rules, Config};
pub use duration::DurationWrapper;
use include_dir::{include_dir, Dir};
use lazy_static::lazy_static;
pub use methods::MethodMatcher;
pub use pass_through::PassThrough;
pub use path::{MatchPathSegment, MatchPathSingleSegment, MatchingPath};
pub use path_modify::PathSegmentsModify;
pub use path_segment::UrlPathSegment;
pub use post_processing::{Encoding, PostProcessing};
pub use project_config::{ProjectConfig, ProjectHandler, ProjectHandlerVariant};
pub use proxy::Proxy;
pub use query::{MatchQuerySingleValue, MatchQueryValue, QueryMatcher};
pub use rebase::Rebase;
pub use response::{RawResponse, RedirectResponse, ResponseBody, StaticResponse, TemplateEngine};
pub use rule::{
    Action, Filter, ModifyHeaders, OnResponse, RequestModifications, ResponseModifications, Rule,
    TrailingSlashFilterRule, TrailingSlashModification,
};
pub use scope::Scope;
use semver::{Version, VersionReq};
pub use static_dir::StaticDir;
pub use status_code::{StatusCode, StatusCodeRange};
pub use upstream::{Probe, UpstreamDefinition, UpstreamSocketAddr};
pub use version::ConfigVersion;

mod application_firewall;
mod auth;
mod cache;
mod catch;
mod client_config;
mod config;
mod duration;
mod gcs;
mod methods;
mod pass_through;
mod path;
mod path_modify;
mod path_segment;
mod post_processing;
mod project_config;
mod proxy;
mod query;
// mod query_modify;
mod rebase;
mod redirect;
pub mod referenced;
pub mod refinable;
mod response;
mod rule;
mod s3;
mod schema;
mod scope;
mod static_dir;
mod status_code;
mod upstream;
mod version;

pub const DEFAULT_CONFIG_FILE: &str = "Exofile.yml";
static CONFIG_SCHEMAS: Dir = include_dir!("config-schemas/schemas");

lazy_static! {
    pub static ref MIN_SUPPORTED_VERSION: Version = "1.0.0-pre.1".parse().unwrap();
    pub static ref CURRENT_VERSION: ConfigVersion = ConfigVersion("1.0.0-pre.1".parse().unwrap());
    pub static ref VERSION_REQUIREMENT: VersionReq = format!(
        ">={} <={} <2",
        MIN_SUPPORTED_VERSION.to_string(),
        CURRENT_VERSION.to_string()
    )
    .parse()
    .unwrap();
}

pub fn is_version_supported(version: &Version) -> bool {
    VERSION_REQUIREMENT.matches(version)
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

pub fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    v == &Default::default()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn test_schema() {
        serde_json::to_string_pretty(&schemars::schema_for!(ClientConfig)).unwrap();
    }

    #[test]
    fn test_version_supported() {
        assert!(!is_version_supported(&"0.2.4".parse().unwrap()));
        assert!(is_version_supported(&"1.0.0-pre.1".parse().unwrap()));
        assert!(!is_version_supported(&"1.0.0-pre.2".parse().unwrap()));
        assert!(!is_version_supported(&"1.23.1".parse().unwrap()));
        assert!(!is_version_supported(&"2.0.0".parse().unwrap()));
    }
}
