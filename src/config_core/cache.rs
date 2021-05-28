use crate::{config_core::Filter, entities::InvalidationGroupName};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Hash, PartialEq, JsonSchema)]
pub struct Cache {
    pub enabled: bool,

    #[serde(default)]
    pub invalidations: Vec<Invalidation>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, PartialEq, JsonSchema)]
pub struct Invalidation {
    pub name: InvalidationGroupName,
    pub filters: Vec<Filter>,
}

impl Default for Cache {
    fn default() -> Self {
        Cache {
            enabled: false,
            invalidations: Default::default(),
        }
    }
}
