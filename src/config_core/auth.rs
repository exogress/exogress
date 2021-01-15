use crate::entities::ParameterName;
use core::fmt;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(deny_unknown_fields)]
pub struct Auth {
    pub providers: Vec<AuthDefinition>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(deny_unknown_fields)]
pub struct AuthDefinition {
    pub name: AuthProvider,
    pub acl: ParameterName,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(deny_unknown_fields)]
pub enum AuthProvider {
    #[serde(rename = "google")]
    Google,

    #[serde(rename = "github")]
    Github,
}

impl FromStr for AuthProvider {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "google" => Ok(AuthProvider::Google),
            "github" => Ok(AuthProvider::Github),
            _ => Err(()),
        }
    }
}

impl fmt::Display for AuthProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthProvider::Google => write!(f, "google"),
            AuthProvider::Github => write!(f, "github"),
        }
    }
}
