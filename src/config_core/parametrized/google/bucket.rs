use crate::config_core::parametrized::{Parameter, ParameterOrConfigValue, ParameterSchema};
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use std::convert::TryFrom;

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(deny_unknown_fields)]
pub struct GcsBucket {
    pub name: SmolStr,
}

impl ParameterOrConfigValue for GcsBucket {
    fn schema() -> ParameterSchema {
        ParameterSchema::GcsBucket
    }
}

impl TryFrom<Parameter> for GcsBucket {
    type Error = ();

    fn try_from(value: Parameter) -> Result<Self, Self::Error> {
        match value {
            Parameter::GcsBucket(bucket) => Ok(bucket),
            _ => Err(()),
        }
    }
}
