use crate::config_core::{
    is_default,
    referenced::{acl::Acl, Container},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema)]
pub struct Auth {
    #[serde(default, skip_serializing_if = "is_default")]
    pub google: Option<GoogleAuthDefinition>,

    #[serde(default, skip_serializing_if = "is_default")]
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
