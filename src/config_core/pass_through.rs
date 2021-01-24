use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, Default)]
#[serde(deny_unknown_fields)]
pub struct PassThrough {}
