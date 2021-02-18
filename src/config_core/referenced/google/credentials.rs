use crate::config_core::referenced::{Parameter, ParameterSchema, ReferencedConfigValue};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use std::convert::TryFrom;

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GoogleCredentials {
    pub json: SmolStr,
}

impl ReferencedConfigValue for GoogleCredentials {
    fn schema() -> ParameterSchema {
        ParameterSchema::GoogleCredentials
    }
}

impl TryFrom<Parameter> for GoogleCredentials {
    type Error = ();

    fn try_from(value: Parameter) -> Result<Self, Self::Error> {
        match value {
            Parameter::GoogleCredentials(creds) => Ok(creds),
            _ => Err(()),
        }
    }
}
