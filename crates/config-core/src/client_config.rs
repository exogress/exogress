use hashbrown::HashSet;

use serde::{Deserialize, Serialize};

use exogress_entities::{
    ConfigName, ExceptionName, HandlerName, MountPointName, StaticResponseName, Upstream,
};

use crate::catch::Catch;
use crate::config::default_rules;
use crate::config::Config;
use crate::path_segment::UrlPathSegmentOrQueryPart;
use crate::proxy::Proxy;
use crate::static_dir::StaticDir;
use crate::upstream::UpstreamDefinition;
use crate::{Auth, ConfigVersion, Rule};
use crate::{StaticResponse, CURRENT_VERSION};
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

#[derive(Debug, Hash, Eq, Serialize, Deserialize, PartialEq, Clone, PartialOrd, Ord)]
#[serde(transparent)]
pub struct ClientConfigRevision(pub u64);

impl From<u64> for ClientConfigRevision {
    fn from(revision: u64) -> Self {
        ClientConfigRevision(revision)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash)]
#[serde(deny_unknown_fields)]
pub struct ClientConfig {
    pub version: ConfigVersion,
    pub revision: ClientConfigRevision,
    pub name: ConfigName,
    #[serde(rename = "mount-points")]
    pub mount_points: BTreeMap<MountPointName, ClientMount>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub upstreams: BTreeMap<Upstream, UpstreamDefinition>,
}

impl ClientConfig {
    pub fn sample(
        config_name: Option<ConfigName>,
        handler_name: Option<HandlerName>,
        mount_point_name: Option<MountPointName>,
        upstream_name: Option<Upstream>,
    ) -> Self {
        let upstream_name = upstream_name.unwrap_or_else(|| "my-upstream".parse().unwrap());
        let mount_point_name =
            mount_point_name.unwrap_or_else(|| "my-mount-point".parse().unwrap());
        let handler_name = handler_name.unwrap_or_else(|| "my-handler".parse().unwrap());
        let config_name = config_name.unwrap_or_else(|| "my-config-name".parse().unwrap());

        let mut upstreams = BTreeMap::new();
        upstreams.insert(
            upstream_name.clone(),
            UpstreamDefinition {
                port: 3000,
                host: None,
                health: vec![],
            },
        );

        let mut handlers = BTreeMap::new();
        handlers.insert(
            handler_name,
            ClientHandler {
                variant: ClientHandlerVariant::Proxy(Proxy {
                    upstream: upstream_name,
                }),
                base_path: vec![],
                replace_base_path: vec![],
                rules: default_rules(),
                priority: 10,
                catch: Default::default(),
            },
        );

        let static_responses = btreemap! {
            // response_name => btreemap!{
            //     "application/html".to_string() => StaticResponse {
            //         content: "<html><body>Not found. Generated at {{ this.time }} </body></html>"
            //             .to_string(),
            //         engine: None,
            //     },
            // }
        };

        let mount_points = btreemap! {
            mount_point_name => ClientMount {
                handlers,
                catch: Default::default(),
                static_responses,
            }
        };

        ClientConfig {
            version: CURRENT_VERSION.clone(),
            revision: 1.into(),
            name: config_name,
            mount_points,
            upstreams,
        }
    }

    pub fn resolve_upstream(&self, upstream: &Upstream) -> Option<UpstreamDefinition> {
        self.upstreams.get(upstream).cloned()
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ClientConfigError {
    #[error("upstreams {} not defined", .0.iter().map(| s | s.to_string()).collect::< Vec < _ >> ().join(", "))]
    UpstreamNotDefined(Vec<Upstream>),

    #[error("mount points {} not defined", .0.iter().map(| s | s.to_string()).collect::< Vec < _ >> ().join(", "))]
    MountPointsNotDefined(Vec<MountPointName>),

    #[error("unsupported config version {}", _0)]
    UnsupportedVersion(ConfigVersion),
}

impl Config for ClientConfig {
    type Error = ClientConfigError;

    fn checksum(&self) -> u64 {
        let mut s = seahash::SeaHasher::new();
        self.hash(&mut s);
        s.finish()
    }

    fn check_mount_points(&self, existing: &[MountPointName]) -> Result<(), ClientConfigError> {
        let used_mount_points = self
            .mount_points
            .keys()
            .collect::<HashSet<&MountPointName>>();
        let existing_mount_points = existing.iter().collect::<HashSet<&MountPointName>>();

        let mut not_defined = used_mount_points
            .difference(&existing_mount_points)
            .peekable();
        if not_defined.peek().is_some() {
            return Err(ClientConfigError::MountPointsNotDefined(
                not_defined.copied().cloned().collect::<Vec<_>>(),
            ));
        }

        Ok(())
    }

    fn validate(&self) -> Result<(), ClientConfigError> {
        if self.version != *CURRENT_VERSION {
            return Err(ClientConfigError::UnsupportedVersion(self.version.clone()));
        }

        let defined_upstreams = self.upstreams.keys().cloned().collect::<HashSet<_>>();
        let used_upstreams = self
            .mount_points
            .values()
            .map(|mount| {
                mount
                    .handlers
                    .values()
                    .filter_map(|handler| match &handler.variant {
                        ClientHandlerVariant::Proxy(proxy) => Some(proxy.upstream.clone()),
                        _ => None,
                    })
            })
            .flatten()
            .collect::<HashSet<Upstream>>();
        let mut not_defined = used_upstreams.difference(&defined_upstreams).peekable();
        if not_defined.peek().is_some() {
            return Err(ClientConfigError::UpstreamNotDefined(
                not_defined.cloned().collect::<Vec<_>>(),
            ));
        }

        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash)]
#[serde(deny_unknown_fields)]
pub struct ClientMount {
    pub handlers: BTreeMap<HandlerName, ClientHandler>,
    #[serde(default)]
    pub catch: BTreeMap<ExceptionName, Catch>,
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        rename = "static-responses"
    )]
    pub static_responses: BTreeMap<StaticResponseName, StaticResponse>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(deny_unknown_fields, tag = "type")]
pub enum ClientHandlerVariant {
    #[serde(rename = "proxy")]
    Proxy(Proxy),

    #[serde(rename = "static-dir")]
    StaticDir(StaticDir),

    #[serde(rename = "auth")]
    Auth(Auth),
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash)]
// FIXME: report bug with enabling `deny_unknown_fields`
// #[serde(deny_unknown_fields)]
pub struct ClientHandler {
    #[serde(flatten)]
    pub variant: ClientHandlerVariant,

    #[serde(default, rename = "base-path")]
    pub base_path: Vec<UrlPathSegmentOrQueryPart>,

    #[serde(default, rename = "replace-base-path")]
    pub replace_base_path: Vec<UrlPathSegmentOrQueryPart>,

    #[serde(default = "default_rules")]
    pub rules: Vec<Rule>,

    pub priority: u16,

    #[serde(default)]
    pub catch: Catch,
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
mount-points:
  mount_point:
    handlers:
      main:
        type: proxy
        priority: 30
        upstream: backend
        base-path: ["my"]
        replace-base-path: []
        rules:
          - filter:
              path: ["a", "b"]
            action:
              kind: invoke
              catch:
                actions:
                  status-codes:
                    - status-codes-range: 5xx
                      action: static-response
                      static-response-name: tmpl
                    - status-codes-range: 3xx
                      action: throw
                      exception: asd
                    - status-codes-range: 200-220
                      action: next-handler
                      set-status-code: 200
          - filter:
              path: ["*"]
            action:
              kind: invoke
    static-responses:
      tmpl:
        kind: raw
        status-code: 200
        headers: {}
        body:
          - content-type: application/html
            content: "<html><body><h1>{{ this.message }}</h1></body>/html>"
            engine: handlebars
      plain:
        kind: raw
        body:
          - content-type: application/html
            content: "<html><body><h1>not found</h1></body>/html>"
"#;
        serde_yaml::from_str::<ClientConfig>(YAML).unwrap();
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
mount-points:
  mount_point:
    handlers:
      main:
        type: proxy
        priority: 30
        upstream: backend
"#;
        let e = serde_yaml::from_str::<ClientConfig>(YAML)
            .unwrap()
            .validate()
            .err()
            .unwrap();

        assert!(matches!(e, ClientConfigError::UpstreamNotDefined(_)));
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
mount-points:
  mount_point:
    handlers:
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
mount-points:
  mount_point:
    handlers:
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
        let c1 = serde_yaml::from_str::<ClientConfig>(YAML1).unwrap();
        let c2 = serde_yaml::from_str::<ClientConfig>(YAML2).unwrap();

        assert_eq!(c1.checksum(), c2.checksum());
    }

    #[test]
    pub fn test_sample() {
        let sample = ClientConfig::sample(None, None, None, None);
        let yaml = serde_yaml::to_string(&sample).unwrap();
        let _: ClientConfig = serde_yaml::from_str(&yaml).unwrap();
    }
}
