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
            methods: Default::default(),
            trailing_slash: Default::default(),
        },
        action: Action::Invoke {
            modify_request: Default::default(),
            modify_response: Default::default(),
            rescue: Default::default(),
        },
    }]
}
