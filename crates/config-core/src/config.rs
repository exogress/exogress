use hashbrown::HashSet;

use semver::Version;
use serde::{Deserialize, Serialize};

use exogress_entities::{ConfigName, MountPointName, TargetName, Upstream};

use crate::path_segment::UrlPathSegmentOrQueryPart;
use crate::proxy::Proxy;
use crate::static_dir::StaticDir;
use std::collections::BTreeMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

pub const ANY_SEGMENTS_MATCH_STR: &str = "*";
pub const ANY_STR: &str = "?";
// pub const REF_STR: &str = "$";

#[derive(Debug, Hash, Eq, Serialize, Deserialize, PartialEq, Clone, PartialOrd, Ord)]
#[serde(transparent)]
pub struct Revision(pub u64);

impl From<u64> for Revision {
    fn from(revision: u64) -> Self {
        Revision(revision)
    }
}

#[derive(Debug, Hash, Eq, Serialize, Deserialize, PartialEq, Clone)]
#[serde(transparent)]
pub struct ConfigVersion(pub Version);

impl fmt::Display for ConfigVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<Version> for ConfigVersion {
    fn from(version: Version) -> Self {
        ConfigVersion(version)
    }
}

impl FromStr for ConfigVersion {
    type Err = semver::SemVerError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Version::parse(s)?.into())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash)]
#[serde(deny_unknown_fields)]
pub struct UpstreamDefinition {
    pub port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    host: Option<String>,
}

impl UpstreamDefinition {
    pub fn on_default_host(port: u16) -> Self {
        UpstreamDefinition { port, host: None }
    }

    pub fn get_host(&self) -> String {
        self.host.clone().unwrap_or_else(|| "127.0.0.1".to_string())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash)]
#[serde(deny_unknown_fields)]
pub struct RootConfig {
    pub version: ConfigVersion,
    pub revision: Revision,
    pub name: ConfigName,
    pub exposes: BTreeMap<MountPointName, Mount>,
    pub upstreams: BTreeMap<Upstream, UpstreamDefinition>,
}

impl RootConfig {
    pub fn sample(
        config_name: Option<ConfigName>,
        target_name: Option<TargetName>,
        mount_point_name: Option<MountPointName>,
        upstream_name: Option<Upstream>,
    ) -> Self {
        let upstream_name = upstream_name.unwrap_or_else(|| "my-upstream".parse().unwrap());
        let mount_point_name =
            mount_point_name.unwrap_or_else(|| "my-mount-point".parse().unwrap());
        let target_name = target_name.unwrap_or_else(|| "my-target".parse().unwrap());
        let config_name = config_name.unwrap_or_else(|| "my-config-name".parse().unwrap());

        let mut upstreams = BTreeMap::new();
        upstreams.insert(
            upstream_name.clone(),
            UpstreamDefinition {
                port: 3000,
                host: None,
            },
        );

        let mut targets = BTreeMap::new();
        targets.insert(
            target_name,
            Target {
                variant: TargetVariant::Proxy(Proxy {
                    upstream: upstream_name,
                }),
                base_path: vec![],
                priority: 10,
            },
        );

        let mut exposes = BTreeMap::new();
        exposes.insert(mount_point_name, Mount { targets });

        RootConfig {
            version: "0.0.1".parse().unwrap(),
            revision: 1.into(),
            name: config_name,
            exposes,
            upstreams,
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ConfigError {
    #[error("upstreams {} not defined", .0.iter().map(| s | s.to_string()).collect::< Vec < _ >> ().join(", "))]
    UpstreamNotDefined(Vec<Upstream>),

    #[error("unsupported config version {}", _0)]
    UnsupportedVersion(ConfigVersion),
}

impl RootConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.version != "0.0.1".parse().unwrap() {
            return Err(ConfigError::UnsupportedVersion(self.version.clone()));
        }

        let defined_upstreams = self.upstreams.keys().cloned().collect::<HashSet<_>>();
        let used_upstreams = self
            .exposes
            .values()
            .map(|mount| {
                mount
                    .targets
                    .values()
                    .filter_map(|target| match &target.variant {
                        TargetVariant::Proxy(proxy) => Some(proxy.upstream.clone()),
                        TargetVariant::StaticDir(_) => None,
                    })
            })
            .flatten()
            .collect::<HashSet<Upstream>>();
        let mut not_defined = used_upstreams.difference(&defined_upstreams).peekable();
        if not_defined.peek().is_some() {
            return Err(ConfigError::UpstreamNotDefined(
                not_defined.cloned().collect::<Vec<_>>(),
            ));
        }

        Ok(())
    }

    pub fn checksum(&self) -> u64 {
        let mut s = seahash::SeaHasher::new();
        self.hash(&mut s);
        s.finish()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash)]
#[serde(deny_unknown_fields)]
pub struct Mount {
    pub targets: BTreeMap<TargetName, Target>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(deny_unknown_fields, tag = "type")]
pub enum TargetVariant {
    #[serde(rename = "proxy")]
    Proxy(Proxy),
    //
    // #[serde(rename = "static_app")]
    // StaticApp(StaticApp),
    //
    #[serde(rename = "static_dir")]
    StaticDir(StaticDir),
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash)]
// FIXME: report bug with enabling `deny_unknown_fields`
// #[serde(deny_unknown_fields)]
pub struct Target {
    #[serde(flatten)]
    pub variant: TargetVariant,
    #[serde(default)]
    pub base_path: Vec<UrlPathSegmentOrQueryPart>,
    pub priority: u16,
    // #[serde(default)]
    // mappings: Vec<Mapping>,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn test_parsing() {
        const YAML: &str = r#"---
version: 0.0.1
revision: 10
name: repository-1
upstreams:
  backend:
    port: 3000
exposes:
  mount_point:
    targets:
      main:
        type: proxy
        priority: 30
        upstream: backend
"#;
        serde_yaml::from_str::<RootConfig>(YAML).unwrap();
    }

    #[test]
    pub fn test_validate_upstream_not_defined() {
        const YAML: &str = r#"---
version: 0.0.1
revision: 10
name: repository-1
upstreams:
  backend2: 
    port: 3000
exposes:
  mount_point:
    targets:
      main:
        type: proxy
        priority: 30
        upstream: backend
"#;
        let e = serde_yaml::from_str::<RootConfig>(YAML)
            .unwrap()
            .validate()
            .err()
            .unwrap();

        assert!(matches!(e, ConfigError::UpstreamNotDefined(_)));
    }

    #[test]
    pub fn test_checksum() {
        const YAML1: &str = r#"---
version: 0.0.1
revision: 10
name: repository-1
upstreams:
  backend:
    port: 3000
  backend2:
    port: 4000
exposes:
  mount_point:
    targets:
      main:
        type: proxy
        priority: 30
        upstream: backend
      main2:
        type: proxy
        priority: 40
        upstream: backend2
"#;
        const YAML2: &str = r#"---
version: 0.0.1
name: repository-1
revision: 10
exposes:
  mount_point:
    targets:
      main2:
        type: proxy
        priority: 40
        upstream: backend2
      main:
        type: proxy
        priority: 30
        upstream: backend
upstreams:
  backend:
    port: 3000
  backend2:
    port: 4000
"#;
        let c1 = serde_yaml::from_str::<RootConfig>(YAML1).unwrap();
        let c2 = serde_yaml::from_str::<RootConfig>(YAML2).unwrap();

        assert_eq!(c1.checksum(), c2.checksum());
    }
}
