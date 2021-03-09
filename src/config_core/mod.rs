pub use crate::config_core::rule::ModifyQuery;
use crate::entities::ProfileName;
use anyhow::bail;
pub use auth::{Auth, GithubAuthDefinition, GoogleAuthDefinition};
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
pub use redirect::RedirectTo;
pub use response::{RawResponse, RedirectResponse, ResponseBody, StaticResponse, TemplateEngine};
pub use rule::{
    Action, Filter, ModifyHeaders, ModifyQueryStrategy, OnResponse, RequestModifications,
    ResponseModifications, Rule, TrailingSlashFilterRule, TrailingSlashModification,
};
pub use schema::get_schema;
pub use scope::Scope;
use semver::{Version, VersionReq};
use serde::{de::DeserializeOwned, Serialize};
pub use static_dir::StaticDir;
pub use status_code::{StatusCode, StatusCodeRange};
use std::collections::BTreeSet;
pub use upstream::{Probe, UpstreamDefinition, UpstreamSocketAddr};
pub use version::ConfigVersion;

// mod application_firewall;
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
static CONFIG_SCHEMAS: Dir = include_dir!("schemas/config");
static PARAMETERS_SCHEMAS: Dir = include_dir!("schemas/parameters");

lazy_static! {
    pub static ref MIN_SUPPORTED_VERSION: Version = "1.0.0".parse().unwrap();
    pub static ref CURRENT_VERSION: ConfigVersion = ConfigVersion("1.0.0".parse().unwrap());
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

pub fn validate_extra_keys<T: Serialize + DeserializeOwned>(
    deserialized_cfg: &T,
    yaml: impl AsRef<[u8]>,
) -> anyhow::Result<()> {
    let deserialize_schemaless: serde_yaml::Value = serde_yaml::from_slice(yaml.as_ref())?;
    let flatten_tree_schemaless =
        serde_value_flatten::to_flatten_maptree(".", None, &deserialize_schemaless)?;

    let serialized_cfg = serde_yaml::to_vec(deserialized_cfg)?;
    let deserialized_scheme: serde_yaml::Value = serde_yaml::from_slice(serialized_cfg.as_ref())?;
    let flatten_tree_with_scheme =
        serde_value_flatten::to_flatten_maptree(".", None, &deserialized_scheme)?;

    let scheme_keys: BTreeSet<_> = flatten_tree_with_scheme.keys().collect();
    let schemaless_keys: BTreeSet<_> = flatten_tree_schemaless.keys().collect();
    let extra_fields: Vec<_> = schemaless_keys
        .difference(&scheme_keys)
        .map(|v| {
            if let serde_value::Value::String(s) = v {
                s.clone()
            } else {
                "ERROR".to_string()
            }
        })
        .collect();

    if !extra_fields.is_empty() {
        bail!("extra fields found: {:?}", extra_fields);
    }

    Ok(())
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
        assert!(is_version_supported(&"1.0.0".parse().unwrap()));
        assert!(!is_version_supported(&"1.0.0-pre.2".parse().unwrap()));
        assert!(!is_version_supported(&"1.23.1".parse().unwrap()));
        assert!(!is_version_supported(&"2.0.0".parse().unwrap()));
    }
}
