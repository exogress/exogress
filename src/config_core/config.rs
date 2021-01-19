use crate::config_core::rule::Filter;
use crate::config_core::{Action, MatchingPath, Rule};
use crate::entities::MountPointName;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;
use std::hash::Hash;

pub trait Config: Serialize + DeserializeOwned + Debug + Clone + Hash {
    type Error: std::error::Error;

    fn checksum(&self) -> u64;
    fn check_mount_points(&self, existing: &[MountPointName]) -> Result<(), Self::Error>;
    fn validate(&self) -> Result<(), Self::Error>;
}

pub fn default_rules() -> Vec<Rule> {
    vec![Rule {
        filter: Filter {
            path: MatchingPath::Wildcard,
            trailing_slash: Default::default(),
        },
        modify: None,
        action: Action::Invoke {
            rescue: Default::default(),
        },
    }]
}
