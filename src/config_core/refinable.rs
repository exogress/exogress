use crate::{
    config_core::{referenced::ReferencedConfigValue, RescueItem, Scope, StaticResponse},
    entities::StaticResponseName,
};
use anyhow::bail;
use core::fmt;
use schemars::JsonSchema;
use serde::{
    de, de::DeserializeOwned, ser::Error, Deserialize, Deserializer, Serialize, Serializer,
};

use std::{
    collections::{BTreeMap, VecDeque},
    hash::Hash,
    str::FromStr,
};

#[derive(Default, Debug)]
pub struct RefinableSet {
    inner: BTreeMap<Scope, Refinable>,
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct Refined {
    pub static_responses: BTreeMap<StaticResponseName, (StaticResponse, Scope)>,
    pub rescue: VecDeque<(RescueItem, Scope)>,
}

impl RefinableSet {
    pub fn new() -> Self {
        RefinableSet::default()
    }

    pub fn add(&mut self, scope: Scope, refinable: &Refinable) -> anyhow::Result<()> {
        if self
            .inner
            .insert(scope.clone(), refinable.clone())
            .is_some()
        {
            bail!("refinable scope {:?} already added", scope);
        }

        Ok(())
    }

    pub fn joined_for_scope(&self, current_scope: &Scope) -> Refined {
        self.inner
            .range(&Scope::ProjectConfig..=current_scope)
            .filter(|(existing_scope, _)| existing_scope.matches_by_same_entity(current_scope))
            .fold(
                Refined {
                    static_responses: Default::default(),
                    rescue: Default::default(),
                },
                |mut acc, (scope, refinable)| {
                    for (static_resp_name, static_resp) in refinable.static_responses.iter() {
                        acc.static_responses.insert(
                            static_resp_name.clone(),
                            (static_resp.clone(), scope.clone()),
                        );
                    }
                    for rescue_item in refinable.rescue.iter().rev() {
                        acc.rescue.push_front((rescue_item.clone(), scope.clone()));
                    }
                    acc
                },
            )
    }
}

#[derive(Default, Serialize, Deserialize, Clone, Debug, Hash, Eq, PartialEq, JsonSchema)]
// #[schemars(deny_unknown_fields)]
pub struct Refinable {
    #[serde(default, rename = "static-responses")]
    pub static_responses: BTreeMap<StaticResponseName, StaticResponse>,
    #[serde(default)]
    pub rescue: Vec<RescueItem>,
}

pub trait SharedEntity:
    DeserializeOwned
    + Serialize
    + core::fmt::Debug
    + Clone
    + Eq
    + PartialEq
    + std::hash::Hash
    + core::fmt::Display
    + FromStr
    + JsonSchema
{
    type Value: ReferencedConfigValue;

    fn get_refined(&self, refined: &RefinableSet, scope: &Scope) -> Option<(Self::Value, Scope)>;
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, JsonSchema)]
pub struct NonExistingSharedEntity(());

impl SharedEntity for NonExistingSharedEntity {
    type Value = ();

    fn get_refined(&self, _refined: &RefinableSet, _scope: &Scope) -> Option<(Self::Value, Scope)> {
        None
    }
}

impl fmt::Display for NonExistingSharedEntity {
    fn fmt(&self, _f: &mut fmt::Formatter) -> fmt::Result {
        Err(fmt::Error)
    }
}

impl Serialize for NonExistingSharedEntity {
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        Err(S::Error::custom(
            "impossible to serialize - non existing entity",
        ))
    }
}

impl<'de> Deserialize<'de> for NonExistingSharedEntity {
    fn deserialize<D>(_deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        Err(de::Error::custom(
            "impossible to deserialize - non existing entity",
        ))
    }
}

impl FromStr for NonExistingSharedEntity {
    type Err = ();

    fn from_str(_s: &str) -> Result<Self, Self::Err> {
        Err(())
    }
}

impl SharedEntity for StaticResponseName {
    type Value = StaticResponse;

    fn get_refined(&self, refined: &RefinableSet, scope: &Scope) -> Option<(Self::Value, Scope)> {
        refined
            .joined_for_scope(scope)
            .static_responses
            .get(self)
            .cloned()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::config_core::{CatchAction, CatchMatcher, ClientConfigRevision};

    #[test]
    fn test_lookup() {
        let mut set = RefinableSet::new();

        set.add(
            Scope::ProjectConfig,
            &Refinable {
                static_responses: Default::default(),
                rescue: vec![RescueItem {
                    catch: CatchMatcher::Exception("exception_project".parse().unwrap()),
                    handle: CatchAction::NextHandler,
                }],
            },
        )
        .unwrap();

        set.add(
            Scope::ClientConfig {
                config: "config1".parse().unwrap(),
                revision: ClientConfigRevision(1),
            },
            &Refinable {
                static_responses: Default::default(),
                rescue: vec![RescueItem {
                    catch: CatchMatcher::Exception("client-config1".parse().unwrap()),
                    handle: CatchAction::NextHandler,
                }],
            },
        )
        .unwrap();

        set.add(
            Scope::ProjectMount {
                mount_point: "mp1".parse().unwrap(),
            },
            &Refinable {
                static_responses: Default::default(),
                rescue: vec![
                    RescueItem {
                        catch: CatchMatcher::Exception("project-mp1".parse().unwrap()),
                        handle: CatchAction::NextHandler,
                    },
                    RescueItem {
                        catch: CatchMatcher::Exception("project-mp1:ex2".parse().unwrap()),
                        handle: CatchAction::NextHandler,
                    },
                ],
            },
        )
        .unwrap();

        assert_eq!(
            set.joined_for_scope(&Scope::ClientMount {
                config: "config1".parse().unwrap(),
                revision: ClientConfigRevision(1),
                mount_point: "mp1".parse().unwrap(),
            }),
            Refined {
                static_responses: Default::default(),
                rescue: vec![
                    (
                        RescueItem {
                            catch: CatchMatcher::Exception("project-mp1".parse().unwrap()),
                            handle: CatchAction::NextHandler,
                        },
                        Scope::ProjectMount {
                            mount_point: "mp1".parse().unwrap()
                        }
                    ),
                    (
                        RescueItem {
                            catch: CatchMatcher::Exception("project-mp1:ex2".parse().unwrap()),
                            handle: CatchAction::NextHandler,
                        },
                        Scope::ProjectMount {
                            mount_point: "mp1".parse().unwrap()
                        }
                    ),
                    (
                        RescueItem {
                            catch: CatchMatcher::Exception("client-config1".parse().unwrap()),
                            handle: CatchAction::NextHandler,
                        },
                        Scope::ClientConfig {
                            config: "config1".parse().unwrap(),
                            revision: ClientConfigRevision(1),
                        }
                    ),
                    (
                        RescueItem {
                            catch: CatchMatcher::Exception("exception_project".parse().unwrap()),
                            handle: CatchAction::NextHandler,
                        },
                        Scope::ProjectConfig
                    ),
                ]
                .into()
            }
        );

        assert_eq!(
            set.joined_for_scope(&Scope::ProjectConfig),
            Refined {
                static_responses: Default::default(),
                rescue: vec![(
                    RescueItem {
                        catch: CatchMatcher::Exception("exception_project".parse().unwrap()),
                        handle: CatchAction::NextHandler,
                    },
                    Scope::ProjectConfig
                )]
                .into()
            }
        );

        assert_eq!(
            set.joined_for_scope(&Scope::ClientConfig {
                config: "config1".parse().unwrap(),
                revision: ClientConfigRevision(1),
            }),
            Refined {
                static_responses: Default::default(),
                rescue: vec![
                    (
                        RescueItem {
                            catch: CatchMatcher::Exception("client-config1".parse().unwrap()),
                            handle: CatchAction::NextHandler,
                        },
                        Scope::ClientConfig {
                            config: "config1".parse().unwrap(),
                            revision: ClientConfigRevision(1),
                        }
                    ),
                    (
                        RescueItem {
                            catch: CatchMatcher::Exception("exception_project".parse().unwrap()),
                            handle: CatchAction::NextHandler,
                        },
                        Scope::ProjectConfig
                    ),
                ]
                .into()
            }
        );
    }
}
