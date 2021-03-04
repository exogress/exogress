use crate::config_core::referenced::{acl::Acl, Container};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema)]
// #[schemars(deny_unknown_fields)]
pub struct Auth {
    #[serde(default)]
    pub google: Option<GoogleAuthDefinition>,

    #[serde(default)]
    pub github: Option<GithubAuthDefinition>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema)]
pub struct GoogleAuthDefinition {
    pub acl: Container<Acl>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema)]
pub struct GithubAuthDefinition {
    pub acl: Container<Acl>,
}
