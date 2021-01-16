use crate::config_core::parametrized::{Parameter, ParameterOrConfigValue, ParameterSchema};
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use std::convert::TryFrom;

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(transparent)]
pub struct Acl(pub Vec<AclEntry>);

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(untagged)]
pub enum AclEntry {
    Allow {
        #[serde(rename = "allow")]
        identity: SmolStr,
    },
    Deny {
        #[serde(rename = "deny")]
        identity: SmolStr,
    },
}

impl ParameterOrConfigValue for Acl {
    fn schema() -> ParameterSchema {
        ParameterSchema::Acl
    }
}

impl TryFrom<Parameter> for Acl {
    type Error = ();

    fn try_from(value: Parameter) -> Result<Self, Self::Error> {
        match value {
            Parameter::Acl(acl) => Ok(acl),
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acl_entry() {
        serde_yaml::from_str::<Acl>(
            r#"
---
- deny: "*@domain.tld"
- allow: "*"
"#,
        )
        .unwrap();
    }
}
