use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(deny_unknown_fields)]
pub struct AwsCredentials {
    pub access_key_id: SmolStr,
    pub secret_access_key: SmolStr,
}
