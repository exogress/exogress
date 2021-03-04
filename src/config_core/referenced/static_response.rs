use crate::config_core::{
    referenced::{Parameter, ParameterSchema, ReferencedConfigValue},
    StaticResponse,
};
use std::convert::TryFrom;

impl TryFrom<Parameter> for StaticResponse {
    type Error = ();

    fn try_from(value: Parameter) -> Result<Self, Self::Error> {
        match value {
            Parameter::StaticResponse(resp) => Ok(*resp),
            _ => Err(()),
        }
    }
}

impl ReferencedConfigValue for StaticResponse {
    fn schema() -> ParameterSchema {
        ParameterSchema::StaticResponse
    }
}
