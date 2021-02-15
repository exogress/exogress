use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq, schemars::JsonSchema)]
pub struct Cache {
    pub enabled: bool,
}

impl Default for Cache {
    fn default() -> Self {
        Cache { enabled: false }
    }
}
