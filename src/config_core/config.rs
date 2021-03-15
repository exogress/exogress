use crate::{
    config_core::{rule::Filter, Action, MatchingPath, QueryMatcher, Rule},
    entities::MountPointName,
};
use serde::{de::DeserializeOwned, Serialize};
use std::{fmt::Debug, hash::Hash};

pub trait Config: Serialize + DeserializeOwned + Debug + Clone + Hash {
    type Error: std::error::Error;

    fn checksum(&self) -> u64;
    fn check_mount_points(&self, existing: &[MountPointName]) -> Result<(), Self::Error>;
    fn validate(&self) -> Result<(), Self::Error>;
    fn parse(yaml: impl AsRef<[u8]>) -> anyhow::Result<Self>;
}

pub fn default_rules() -> Vec<Rule> {
    vec![Rule {
        filter: Filter {
            path: MatchingPath::Wildcard,
            query_params: QueryMatcher {
                inner: Default::default(),
            },
            methods: Default::default(),
            trailing_slash: Default::default(),
        },
        action: Action::Invoke {
            modify_request: Default::default(),
            on_response: Default::default(),
            rescue: Default::default(),
        },
        profiles: None,
    }]
}
