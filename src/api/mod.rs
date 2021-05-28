use crate::entities::{ConfigName, HandlerName, InvalidationGroupName, MountPointName};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SingleInvalidationRequest {
    pub invalidation_name: InvalidationGroupName,
    pub mount_point_name: MountPointName,
    pub handler_name: HandlerName,
    pub config_name: Option<ConfigName>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InvalidationRequest {
    pub groups: Vec<SingleInvalidationRequest>,
}
