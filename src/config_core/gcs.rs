use crate::entities::ParameterName;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(deny_unknown_fields)]
pub struct GcsBucket {
    pub bucket: ParameterName,
    pub credentials: ParameterName,
}
