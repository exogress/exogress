use serde::{Deserialize, Serialize};

#[derive(
    Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, Default, schemars::JsonSchema,
)]
pub struct PassThrough {}
