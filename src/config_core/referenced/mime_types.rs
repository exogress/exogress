use crate::{
    config_core::referenced::{Parameter, ParameterSchema, ReferencedConfigValue},
    entities::schemars::{gen::SchemaGenerator, schema::Schema},
};
use mime::Mime;
use schemars::{
    schema::{InstanceType, Metadata, SchemaObject, StringValidation},
    JsonSchema,
};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use std::convert::TryFrom;

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq, JsonSchema)]
#[serde(transparent)]
pub struct MimeTypes(pub Vec<MimeType>);

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
#[serde(transparent)]
pub struct MimeType(#[serde_as(as = "DisplayFromStr")] pub mime::Mime);

impl From<mime::Mime> for MimeType {
    fn from(inner: Mime) -> Self {
        MimeType(inner)
    }
}

impl JsonSchema for MimeType {
    fn schema_name() -> String {
        format!("MimeType")
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        SchemaObject {
            metadata: Some(Box::new(Metadata {
                title: Some(String::from("mime-type")),
                ..Default::default()
            })),
            string: Some(Box::new(StringValidation {
                max_length: None,
                min_length: None,
                pattern: None,
            })),
            instance_type: Some(InstanceType::String.into()),
            ..Default::default()
        }
        .into()
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn test_schema() {
        serde_json::to_string_pretty(&schemars::schema_for!(MimeTypes)).unwrap();
        serde_json::to_string_pretty(&schemars::schema_for!(MimeType)).unwrap();
    }
}
