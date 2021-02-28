use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, Default, JsonSchema)]
// #[schemars(deny_unknown_fields)]
pub struct PassThrough {}
