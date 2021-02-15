use crate::{
    config_core::referenced::{Parameter, ParameterSchema, ReferencedConfigValue},
    entities::schemars::{gen::SchemaGenerator, schema::Schema},
};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use std::convert::TryFrom;

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
#[serde(transparent)]
pub struct MimeTypes(#[serde_as(as = "Vec<DisplayFromStr>")] pub Vec<mime::Mime>);

impl schemars::JsonSchema for MimeTypes {
    fn schema_name() -> String {
        unimplemented!()
    }

    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        unimplemented!()
    }
}

impl TryFrom<Parameter> for MimeTypes {
    type Error = ();

    fn try_from(value: Parameter) -> Result<Self, Self::Error> {
        match value {
            Parameter::MimeTypes(mime_types) => Ok(mime_types),
            _ => Err(()),
        }
    }
}

impl ReferencedConfigValue for MimeTypes {
    fn schema() -> ParameterSchema {
        ParameterSchema::MimeTypes
    }
}
