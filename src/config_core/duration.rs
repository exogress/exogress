use schemars::{
    gen::SchemaGenerator,
    schema::{InstanceType, Schema, SchemaObject},
    JsonSchema,
};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
#[serde(transparent)]
pub struct DurationWrapper(#[serde(with = "humantime_serde")] pub std::time::Duration);

impl JsonSchema for DurationWrapper {
    fn schema_name() -> String {
        "Duration".to_string()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        SchemaObject {
            instance_type: Some(InstanceType::String.into()),
            ..Default::default()
        }
        .into()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn test_schema() {
        let s = serde_json::to_string_pretty(&schemars::schema_for!(DurationWrapper)).unwrap();
        println!("{}", s);
    }
}
