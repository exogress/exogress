use crate::{config_core::Filter, entities::InvalidationGroupName};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Debug, Clone, Hash, PartialEq, JsonSchema)]
pub struct Cache {
    pub enabled: bool,

    #[serde(default)]
    pub invalidations: BTreeMap<InvalidationGroupName, Vec<Filter>>,
}

impl Default for Cache {
    fn default() -> Self {
        Cache {
            enabled: false,
            invalidations: Default::default(),
        }
    }
}
