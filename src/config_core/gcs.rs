use crate::config_core::parametrized::google::bucket::GcsBucket;
use crate::config_core::parametrized::google::credentials::GoogleCredentials;
use crate::config_core::parametrized::Container;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(deny_unknown_fields)]
pub struct GcsBucketAccess {
    pub bucket: Container<GcsBucket>,
    pub credentials: Container<GoogleCredentials>,
}
