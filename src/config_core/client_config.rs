use hashbrown::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::{
    config_core::{
        cache::Cache,
        config::{default_rules, Config},
        gcs::GcsBucketAccess,
        is_profile_active, is_version_supported,
        proxy::Proxy,
        proxy_public::ProxyPublic,
        rebase::Rebase,
        refinable::Refinable,
        s3::S3BucketAccess,
        schema::validate_schema,
        static_dir::StaticDir,
        upstream::{ProbeError, UpstreamDefinition, UpstreamSocketAddr},
        validate_extra_keys, Auth, ConfigVersion, PassThrough, Rule, CURRENT_VERSION,
    },
    entities::{
        ConfigName, HandlerName, HealthCheckProbeName, MountPointName, ProfileName, Upstream,
    },
};
use core::fmt;
use maplit::btreemap;
use schemars::JsonSchema;
use std::{
    collections::BTreeMap,
    fmt::Formatter,
    hash::{Hash, Hasher},
    mem,
};

#[derive(
    Debug, Hash, Eq, Serialize, Deserialize, PartialEq, Clone, PartialOrd, Ord, Copy, JsonSchema,
)]
#[serde(transparent)]
pub struct ClientConfigRevision(pub u64);

impl fmt::Display for ClientConfigRevision {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<u64> for ClientConfigRevision {
    fn from(revision: u64) -> Self {
        ClientConfigRevision(revision)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, JsonSchema)]
// #[schemars(deny_unknown_fields)]
pub struct ClientConfig {
    pub version: ConfigVersion,

    pub revision: ClientConfigRevision,

    pub name: ConfigName,

    #[serde(rename = "mount-points")]
    pub mount_points: BTreeMap<MountPointName, ClientMount>,

    #[serde(default)]
    pub upstreams: BTreeMap<Upstream, UpstreamDefinition>,

    #[serde(flatten)]
    pub refinable: Refinable,
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
                addr: UpstreamSocketAddr {
                    port: 3000,
                    host: None,
                },
                health_checks: Default::default(),
                profiles: None,
            },
        );

        let mut handlers = BTreeMap::new();
        handlers.insert(
            handler_name,
            ClientHandler {
                variant: ClientHandlerVariant::Proxy(Proxy {
                    upstream: upstream_name,
                    rebase: Rebase {
                        base_path: vec![],
                        replace_base_path: vec![],
                    },
                    cache: Default::default(),
                    post_processing: Default::default(),
                    websockets: true,
                }),
                rules: default_rules(),
                priority: 10,
                refinable: Refinable {
                    static_responses: Default::default(),
                    rescue: vec![],
                },
                profiles: None,
                languages: None,
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
                profiles: Default::default(),
                refinable: Refinable {
                    rescue: Default::default(),
                    static_responses,
                }
            }
        };

        ClientConfig {
            version: CURRENT_VERSION.clone(),
            revision: 1.into(),
            name: config_name,
            mount_points,
            upstreams,
            refinable: Refinable {
                static_responses: Default::default(),
                rescue: vec![],
            },
        }
    }

    pub fn parse_with_redefined_upstreams(
        yaml: impl AsRef<[u8]>,
        redefined_upstreams: &HashMap<Upstream, UpstreamSocketAddr>,
    ) -> anyhow::Result<Self> {
        let mut cfg = Self::parse(yaml.as_ref())?;

        for (upstream_name, addr) in redefined_upstreams {
            let upstream = cfg.upstreams.get_mut(upstream_name);
            if let Some(definition) = upstream {
                let _ = mem::replace(&mut definition.addr, addr.clone());
            }
        }

        Ok(cfg)
    }

    pub fn resolve_upstream(
        &self,
        upstream: &Upstream,
        active_profile: &Option<ProfileName>,
    ) -> Option<UpstreamDefinition> {
        self.upstreams
            .get(upstream)
            .filter(|upstream_definition| {
                is_profile_active(&upstream_definition.profiles, active_profile)
            })
            .cloned()
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

    #[error("bad health check values on probe {probe_name}: {probe_error}")]
    BadHealthCheckValues {
        probe_name: HealthCheckProbeName,
        probe_error: ProbeError,
    },
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
        if !is_version_supported(&self.version.0) {
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

        self.upstreams
            .values()
            .map(|upstream| {
                upstream
                    .health_checks
                    .iter()
                    .map(|(probe_name, probe)| {
                        probe.validate().map_err(|probe_validation_error| {
                            ClientConfigError::BadHealthCheckValues {
                                probe_name: probe_name.clone(),
                                probe_error: probe_validation_error,
                            }
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;

        let mut not_defined = used_upstreams.difference(&defined_upstreams).peekable();
        if not_defined.peek().is_some() {
            return Err(ClientConfigError::UpstreamNotDefined(
                not_defined.cloned().collect::<Vec<_>>(),
            ));
        }

        Ok(())
    }

    fn parse(yaml: impl AsRef<[u8]>) -> anyhow::Result<Self> {
        let deserialized_cfg = serde_yaml::from_slice::<Self>(yaml.as_ref())?;

        validate_extra_keys(&deserialized_cfg, yaml.as_ref())?;
        validate_schema(yaml.as_ref(), "client.json")?;

        Ok(deserialized_cfg)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, JsonSchema)]
// #[schemars(deny_unknown_fields)]
pub struct ClientMount {
    #[serde(default)]
    pub handlers: BTreeMap<HandlerName, ClientHandler>,

    #[serde(default)]
    pub profiles: Option<Vec<ProfileName>>,

    #[serde(flatten)]
    pub refinable: Refinable,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Hash, JsonSchema)]
// #[schemars(deny_unknown_fields)]
#[serde(tag = "kind")]
pub enum ClientHandlerVariant {
    #[serde(rename = "proxy")]
    Proxy(Proxy),

    #[serde(rename = "proxy-public")]
    ProxyPublic(ProxyPublic),

    #[serde(rename = "static-dir")]
    StaticDir(StaticDir),

    #[serde(rename = "auth")]
    Auth(Auth),

    #[serde(rename = "s3-bucket")]
    S3Bucket(S3BucketAccess),

    #[serde(rename = "gcs-bucket")]
    GcsBucket(GcsBucketAccess),

    // #[serde(rename = "application-firewall")]
    // ApplicationFirewall(ApplicationFirewall),
    #[serde(rename = "pass-through")]
    PassThrough(PassThrough),
}

impl ClientHandlerVariant {
    pub fn rebase(&self) -> Option<&Rebase> {
        match self {
            ClientHandlerVariant::Proxy(v) => Some(&v.rebase),
            ClientHandlerVariant::StaticDir(v) => Some(&v.rebase),
            ClientHandlerVariant::Auth(_) => None,
            ClientHandlerVariant::S3Bucket(v) => Some(&v.rebase),
            ClientHandlerVariant::GcsBucket(v) => Some(&v.rebase),
            ClientHandlerVariant::PassThrough(_) => None,
            ClientHandlerVariant::ProxyPublic(v) => Some(&v.rebase),
        }
    }

    pub fn cache(&self) -> Option<&Cache> {
        match self {
            ClientHandlerVariant::Proxy(v) => Some(&v.cache),
            ClientHandlerVariant::StaticDir(v) => Some(&v.cache),
            ClientHandlerVariant::Auth(_) => None,
            ClientHandlerVariant::S3Bucket(v) => Some(&v.cache),
            ClientHandlerVariant::GcsBucket(v) => Some(&v.cache),
            ClientHandlerVariant::PassThrough(_) => None,
            ClientHandlerVariant::ProxyPublic(v) => Some(&v.cache),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, JsonSchema)]
pub struct Languages {
    pub supported: Vec<language_tags::LanguageTag>,
    pub default: Option<language_tags::LanguageTag>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, JsonSchema)]
pub struct ClientHandler {
    #[serde(flatten)]
    pub variant: ClientHandlerVariant,

    #[serde(default = "default_rules")]
    pub rules: Vec<Rule>,

    pub priority: u16,

    #[serde(flatten)]
    pub refinable: Refinable,

    #[serde(default)]
    pub profiles: Option<Vec<ProfileName>>,

    #[serde(default)]
    pub languages: Option<Languages>,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn test_language() {
        const YAML: &str = r#"---
version: 1.1.0
revision: 10
name: repository-1
upstreams:
  backend2: 
    port: 3000
mount-points:
  mount_point:
    handlers:
      main:
        kind: pass-through
        priority: 30
        languages: 
          supported: ["ru", "en", "en-US", "tr"]
"#;
        ClientConfig::parse_with_redefined_upstreams(YAML, &Default::default())
            .unwrap()
            .validate()
            .unwrap();
    }

    #[test]
    pub fn test_parsing_1_0_0() {
        const YAML: &str = r#"---
version: 1.0.0
revision: 10
name: repository-1
upstreams:
  backend:
    port: 3000
mount-points:
  mount_point:
    handlers:
      auth:
        kind: auth
        priority: 10
        github:
          acl:
            - allow: glebpom
            - deny: "*"
      main:
        kind: proxy
        priority: 30
        upstream: backend
        base-path: ["my"]
        replace-base-path: []
        rules:
          - filter:
              path: ["a", "b"]
            action: invoke
            rescue:
              - catch: "status-code:5xx"
                action: respond
                static-response: tmpl
              - catch: "status-code:3xx"
                action: throw
                exception: asd
              - catch: "status-code:200-220"
                action: next-handler
          - filter:
              path: ["*"]
            action: invoke
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
        ClientConfig::parse_with_redefined_upstreams(YAML, &Default::default()).unwrap();
    }

    #[test]
    pub fn test_parsing_1_1_0() {
        const YAML: &str = r#"---
version: 1.1.0
revision: 10
name: repository-1
mount-points:
  mount_point:
    handlers:
      main:
        kind: proxy-public
        priority: 30
        host: google.com
"#;
        ClientConfig::parse_with_redefined_upstreams(YAML, &Default::default()).unwrap();
    }

    #[test]
    pub fn test_validate_upstream_not_defined() {
        const YAML: &str = r#"---
version: 1.0.0
revision: 10
name: repository-1
upstreams:
  backend2: 
    port: 3000
mount-points:
  mount_point:
    handlers:
      main:
        kind: proxy
        priority: 30
        upstream: backend
"#;
        let e = ClientConfig::parse_with_redefined_upstreams(YAML, &Default::default())
            .unwrap()
            .validate()
            .err()
            .unwrap();

        assert!(matches!(e, ClientConfigError::UpstreamNotDefined(_)));
    }

    #[test]
    pub fn test_checksum() {
        const YAML1: &str = r#"---
version: 1.0.0
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
        kind: proxy
        priority: 30
        upstream: backend
      main2:
        kind: proxy
        priority: 40
        upstream: backend2
"#;
        const YAML2: &str = r#"---
version: 1.0.0
name: repository-1
revision: 10
mount-points:
  mount_point:
    handlers:
      main2:
        kind: proxy
        priority: 40
        upstream: backend2
      main:
        kind: proxy
        priority: 30
        upstream: backend
upstreams:
  backend:
    port: 3000
  backend2:
    port: 4000
"#;
        let c1 = ClientConfig::parse_with_redefined_upstreams(YAML1, &Default::default()).unwrap();
        let c2 = ClientConfig::parse_with_redefined_upstreams(YAML2, &Default::default()).unwrap();

        assert_eq!(c1.checksum(), c2.checksum());
    }

    #[test]
    pub fn test_sample() {
        let sample = ClientConfig::sample(None, None, None, None);
        let yaml = serde_yaml::to_string(&sample).unwrap();
        let _: ClientConfig = serde_yaml::from_str(&yaml).unwrap();
    }
}
