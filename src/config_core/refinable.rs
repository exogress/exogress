use crate::{
    config_core::{referenced::Container, RescueItem, StaticResponse},
    entities::StaticResponseName,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Clone, Debug, Hash, Eq, PartialEq)]
pub struct Refinable {
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        rename = "static-responses"
    )]
    pub static_responses: BTreeMap<StaticResponseName, StaticResponse>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rescue: Vec<RescueItem>,
}
