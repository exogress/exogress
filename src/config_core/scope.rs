use crate::{
    config_core::ClientConfigRevision,
    entities::{ConfigName, HandlerName, MountPointName},
};
use seahash::SeaHasher;
use std::{
    cmp::Ordering,
    hash::{Hash, Hasher},
};

#[derive(Eq, PartialEq, Clone, Debug, Hash)]
pub enum Scope {
    ProjectConfig,
    ClientConfig {
        config: ConfigName,
        revision: ClientConfigRevision,
    },
    ProjectMount {
        mount_point: MountPointName,
    },
    ClientMount {
        config: ConfigName,
        revision: ClientConfigRevision,
        mount_point: MountPointName,
    },
    ProjectHandler {
        mount_point: MountPointName,
        handler: HandlerName,
    },
    ClientHandler {
        config: ConfigName,
        revision: ClientConfigRevision,
        mount_point: MountPointName,
        handler: HandlerName,
    },
    ProjectRule {
        mount_point: MountPointName,
        handler: HandlerName,
        rule_num: usize,
    },
    ClientRule {
        config: ConfigName,
        revision: ClientConfigRevision,
        mount_point: MountPointName,
        handler: HandlerName,
        rule_num: usize,
    },
}

impl Scope {
    pub fn prev(
        &self,
        client_config_info: &Option<(ConfigName, ClientConfigRevision)>,
    ) -> Option<Scope> {
        match self {
            Scope::ProjectConfig => None,
            Scope::ClientConfig { .. } => Some(Scope::ProjectConfig),
            Scope::ProjectMount { .. } => {
                if let Some((config, revision)) = client_config_info {
                    Some(Scope::ClientConfig {
                        config: config.clone(),
                        revision: *revision,
                    })
                } else {
                    Some(Scope::ProjectConfig)
                }
            }
            Scope::ClientMount { mount_point, .. } => Some(Scope::ProjectMount {
                mount_point: mount_point.clone(),
            }),
            Scope::ProjectHandler { mount_point, .. } => {
                if let Some((config, revision)) = client_config_info {
                    Some(Scope::ClientMount {
                        config: config.clone(),
                        revision: *revision,
                        mount_point: mount_point.clone(),
                    })
                } else {
                    Some(Scope::ProjectMount {
                        mount_point: mount_point.clone(),
                    })
                }
            }
            Scope::ClientHandler {
                mount_point,
                handler,
                ..
            } => Some(Scope::ProjectHandler {
                mount_point: mount_point.clone(),
                handler: handler.clone(),
            }),
            Scope::ProjectRule {
                mount_point,
                handler,
                ..
            } => {
                if let Some((config, revision)) = client_config_info {
                    Some(Scope::ClientHandler {
                        config: config.clone(),
                        revision: *revision,
                        mount_point: mount_point.clone(),
                        handler: handler.clone(),
                    })
                } else {
                    Some(Scope::ProjectHandler {
                        mount_point: mount_point.clone(),
                        handler: handler.clone(),
                    })
                }
            }
            Scope::ClientRule {
                mount_point,
                handler,
                rule_num,
                ..
            } => Some(Scope::ProjectRule {
                mount_point: mount_point.clone(),
                handler: handler.clone(),
                rule_num: *rule_num,
            }),
        }
    }

    pub fn handler(
        client_config: Option<(ConfigName, ClientConfigRevision)>,
        mount_point: &MountPointName,
        handler_name: &HandlerName,
    ) -> Scope {
        match client_config {
            None => Scope::ProjectHandler {
                mount_point: mount_point.clone(),
                handler: handler_name.clone(),
            },
            Some((config_name, revision)) => Scope::ClientHandler {
                config: config_name,
                revision,
                mount_point: mount_point.clone(),
                handler: handler_name.clone(),
            },
        }
    }

    pub fn rule(
        client_config: Option<(ConfigName, ClientConfigRevision)>,
        mount_point: &MountPointName,
        handler_name: &HandlerName,
        rule_num: usize,
    ) -> Scope {
        match client_config {
            None => Scope::ProjectRule {
                mount_point: mount_point.clone(),
                handler: handler_name.clone(),
                rule_num,
            },
            Some((config_name, revision)) => Scope::ClientRule {
                config: config_name,
                revision,
                mount_point: mount_point.clone(),
                handler: handler_name.clone(),
                rule_num,
            },
        }
    }

    fn order(&self) -> u8 {
        match self {
            Scope::ProjectConfig => 1,
            Scope::ClientConfig { .. } => 2,
            Scope::ProjectMount { .. } => 3,
            Scope::ClientMount { .. } => 4,
            Scope::ProjectHandler { .. } => 5,
            Scope::ClientHandler { .. } => 6,
            Scope::ProjectRule { .. } => 7,
            Scope::ClientRule { .. } => 8,
        }
    }

    fn mount_point_name(&self) -> Option<&MountPointName> {
        match self {
            Scope::ClientMount { mount_point, .. }
            | Scope::ClientHandler { mount_point, .. }
            | Scope::ClientRule { mount_point, .. } => Some(mount_point),
            _ => None,
        }
    }

    fn config_revision(&self) -> Option<&ClientConfigRevision> {
        match self {
            Scope::ClientMount { revision, .. }
            | Scope::ClientHandler { revision, .. }
            | Scope::ClientRule { revision, .. } => Some(revision),
            _ => None,
        }
    }

    fn config_name(&self) -> Option<&ConfigName> {
        match self {
            Scope::ClientMount { config, .. }
            | Scope::ClientHandler { config, .. }
            | Scope::ClientRule { config, .. } => Some(config),
            _ => None,
        }
    }

    fn handler_name(&self) -> Option<&HandlerName> {
        match self {
            Scope::ClientHandler { handler, .. } | Scope::ClientRule { handler, .. } => {
                Some(handler)
            }
            _ => None,
        }
    }

    fn rule_num(&self) -> Option<&usize> {
        match self {
            Scope::ClientRule { rule_num, .. } => Some(rule_num),
            _ => None,
        }
    }

    fn is_equal_if_both_set_or_any_is_none<T: PartialEq>(a: Option<T>, b: Option<T>) -> bool {
        a.zip(b).map(|(a, b)| a == b).unwrap_or(true)
    }

    pub(crate) fn matches_by_same_entity(&self, match_to: &Scope) -> bool {
        Self::is_equal_if_both_set_or_any_is_none(self.config_name(), match_to.config_name())
            && Self::is_equal_if_both_set_or_any_is_none(
                self.config_revision(),
                match_to.config_revision(),
            )
            && Self::is_equal_if_both_set_or_any_is_none(
                self.mount_point_name(),
                match_to.mount_point_name(),
            )
            && Self::is_equal_if_both_set_or_any_is_none(
                self.handler_name(),
                match_to.handler_name(),
            )
            && Self::is_equal_if_both_set_or_any_is_none(self.rule_num(), match_to.rule_num())
    }
}

impl PartialOrd for Scope {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = SeaHasher::new();
    t.hash(&mut s);
    s.finish()
}

impl Ord for Scope {
    fn cmp(&self, other: &Self) -> Ordering {
        self.order()
            .cmp(&other.order())
            .then_with(|| calculate_hash(self).cmp(&calculate_hash(&other)))
    }
}
