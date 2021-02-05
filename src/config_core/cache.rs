use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Cache {
    pub enabled: bool,
}

impl Default for Cache {
    fn default() -> Self {
        Cache { enabled: false }
    }
}
