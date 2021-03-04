use crate::config_core::referenced::{Parameter, ParameterSchema, ReferencedConfigValue};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use std::convert::TryFrom;

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, JsonSchema)]

pub struct AwsCredentials {
    pub access_key_id: SmolStr,
    pub secret_access_key: SmolStr,
}

impl ReferencedConfigValue for AwsCredentials {
    fn schema() -> ParameterSchema {
        ParameterSchema::AwsCredentials
    }
}

impl TryFrom<Parameter> for AwsCredentials {
    type Error = ();

    fn try_from(value: Parameter) -> Result<Self, Self::Error> {
        match value {
            Parameter::AwsCredentials(creds) => Ok(creds),
            _ => Err(()),
        }
    }
}
