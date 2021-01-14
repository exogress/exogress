use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(deny_unknown_fields)]
pub struct GoogleCredentials {
    // pub json: HashMap<SmolStr, SmolStr>,
    pub json: SmolStr,
}
