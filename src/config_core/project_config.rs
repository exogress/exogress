use hashbrown::HashSet;

use crate::{
    config_core::{
        auth::GoogleAuthDefinition,
        client_config::{ClientMount, Languages},
        config::default_rules,
        gcs::GcsBucketAccess,
        is_version_supported,
        proxy_public::ProxyPublic,
        referenced::Container,
        refinable::Refinable,
        s3::S3BucketAccess,
        schema::validate_schema,
        validate_extra_keys, Auth, ClientHandler, ClientHandlerVariant, Config, ConfigVersion,
        PassThrough, Rule, CURRENT_VERSION,
    },
    entities::{HandlerName, MountPointName},
};
use maplit::btreemap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    hash::{Hash, Hasher},
};

#[derive(Serialize, Deserialize, Debug, Clone, Hash, JsonSchema)]
// #[schemars(deny_unknown_fields)]
pub struct ProjectConfig {
    pub version: ConfigVersion,

    #[serde(rename = "mount-points", default)]
    pub mount_points: BTreeMap<MountPointName, ProjectMount>,

    #[serde(flatten)]
    pub refinable: Refinable,
}

impl ProjectConfig {
    /// Project-level config sample
    pub fn sample(
        handler_name: Option<HandlerName>,
        mount_point_name: Option<MountPointName>,
    ) -> Self {
        let mount_point_name =
            mount_point_name.unwrap_or_else(|| "my-mount-point".parse().unwrap());
        let handler_name = handler_name.unwrap_or_else(|| "my-handler".parse().unwrap());

        let mut handlers = BTreeMap::new();
        handlers.insert(
            handler_name,
            ProjectHandler {
                variant: ProjectHandlerVariant::Auth(Auth {
                    google: Some(GoogleAuthDefinition {
                        acl: Container::Parameter("acl-var".parse().unwrap()),
                    }),
                    github: None,
                }),
                rules: default_rules(),
                priority: 10,
                refinable: Refinable {
                    static_responses: Default::default(),
                    rescue: Default::default(),
                },
                languages: None,
            },
        );

        let mount_points = btreemap! {
            mount_point_name => ProjectMount {
                handlers,
                refinable: Refinable {
                    rescue: Default::default(),
                    static_responses: Default::default(),
                }
            },
        };

        ProjectConfig {
            version: CURRENT_VERSION.clone(),
            mount_points,
            refinable: Refinable {
                rescue: Default::default(),
                static_responses: Default::default(),
            },
        }
    }

    pub fn default_with_mount_point(mount_point_name: &MountPointName) -> Self {
        ProjectConfig {
            version: CURRENT_VERSION.clone(),
            mount_points: btreemap! {
                mount_point_name.clone() => ProjectMount {
                    handlers: Default::default(),
                    refinable: Refinable {
                        static_responses: Default::default(),
                        rescue: Default::default(),
                    }
                }
            },
            refinable: Refinable {
                rescue: Default::default(),
                static_responses: Default::default(),
            },
        }
    }
}

impl Default for ProjectConfig {
    fn default() -> Self {
        ProjectConfig {
            version: CURRENT_VERSION.clone(),
            mount_points: Default::default(),
            refinable: Refinable {
                rescue: Default::default(),
                static_responses: Default::default(),
            },
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ProjectConfigError {
    #[error("mount points {} not defined", .0.iter().map(| s | s.to_string()).collect::< Vec < _ >> ().join(", "))]
    MountPointsNotDefined(Vec<MountPointName>),

    #[error("unsupported config version {}", _0)]
    UnsupportedVersion(ConfigVersion),
}

impl Config for ProjectConfig {
    type Error = ProjectConfigError;

    fn checksum(&self) -> u64 {
        let mut s = seahash::SeaHasher::new();
        self.hash(&mut s);
        s.finish()
    }

    fn check_mount_points(&self, existing: &[MountPointName]) -> Result<(), ProjectConfigError> {
        let used_mount_points = self
            .mount_points
            .keys()
            .collect::<HashSet<&MountPointName>>();
        let existing_mount_points = existing.iter().collect::<HashSet<&MountPointName>>();

        let mut not_defined = used_mount_points
            .difference(&existing_mount_points)
            .peekable();
        if not_defined.peek().is_some() {
            return Err(ProjectConfigError::MountPointsNotDefined(
                not_defined.copied().cloned().collect::<Vec<_>>(),
            ));
        }

        Ok(())
    }

    fn validate(&self) -> Result<(), ProjectConfigError> {
        if !is_version_supported(&self.version.0) {
            return Err(ProjectConfigError::UnsupportedVersion(self.version.clone()));
        }

        Ok(())
    }

    fn parse(yaml: impl AsRef<[u8]>) -> anyhow::Result<Self> {
        let deserialized_cfg = serde_yaml::from_slice::<Self>(yaml.as_ref())?;

        validate_extra_keys(&deserialized_cfg, yaml.as_ref())?;
        validate_schema(yaml.as_ref(), "project.json")?;

        Ok(deserialized_cfg)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, JsonSchema)]
// #[schemars(deny_unknown_fields)]
pub struct ProjectMount {
    #[serde(default)]
    pub handlers: BTreeMap<HandlerName, ProjectHandler>,

    #[serde(flatten)]
    pub refinable: Refinable,
}

impl From<ProjectMount> for ClientMount {
    fn from(m: ProjectMount) -> Self {
        ClientMount {
            handlers: m.handlers.into_iter().map(|(k, v)| (k, v.into())).collect(),
            profiles: None,
            refinable: m.refinable,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Hash, JsonSchema)]
#[serde(tag = "kind")]
pub enum ProjectHandlerVariant {
    #[serde(rename = "auth")]
    Auth(Auth),

    #[serde(rename = "proxy-public")]
    ProxyPublic(ProxyPublic),

    #[serde(rename = "s3-bucket")]
    S3Bucket(S3BucketAccess),

    #[serde(rename = "gcs-bucket")]
    GcsBucket(GcsBucketAccess),

    // #[serde(rename = "application-firewall")]
    // ApplicationFirewall(ApplicationFirewall),
    #[serde(rename = "pass-through")]
    PassThrough(PassThrough),
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, JsonSchema)]
pub struct ProjectHandler {
    #[serde(flatten)]
    pub variant: ProjectHandlerVariant,

    #[serde(default = "default_rules")]
    pub rules: Vec<Rule>,

    pub priority: u16,

    #[serde(flatten)]
    pub refinable: Refinable,

    #[serde(default)]
    pub languages: Option<Languages>,
}

impl From<ProjectHandler> for ClientHandler {
    fn from(f: ProjectHandler) -> Self {
        let v = match f.variant {
            ProjectHandlerVariant::Auth(auth) => ClientHandlerVariant::Auth(auth),
            ProjectHandlerVariant::S3Bucket(s3_bucket) => ClientHandlerVariant::S3Bucket(s3_bucket),
            ProjectHandlerVariant::GcsBucket(gcs_bucket) => {
                ClientHandlerVariant::GcsBucket(gcs_bucket)
            }
            // ProjectHandlerVariant::ApplicationFirewall(app_firewall) => {
            //     ClientHandlerVariant::ApplicationFirewall(app_firewall)
            // }
            ProjectHandlerVariant::PassThrough(pass_through) => {
                ClientHandlerVariant::PassThrough(pass_through)
            }
            ProjectHandlerVariant::ProxyPublic(proxy_public) => {
                ClientHandlerVariant::ProxyPublic(proxy_public)
            }
        };
        ClientHandler {
            variant: v,
            rules: f.rules,
            priority: f.priority,
            refinable: f.refinable,
            profiles: None,
            languages: f.languages,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn test_parsing() {
        const YAML: &str = r#"---
version: 1.0.0
mount-points:
  mount_point:
    handlers:
      main:
        kind: auth
        priority: 30
        github:
          acl: "@my-acl"
    static-responses:
      redirect:
        kind: redirect
        redirect-type: moved-permanently
        destination: "https://google.com/"
        headers: 
          x-redirected: "1"
    rescue:
      - catch: status-code:5xx
        action: respond
        static-response: tmpl
        data:
          custom: info
"#;
        ProjectConfig::parse(YAML).unwrap();
    }

    #[test]
    pub fn test_sample() {
        let sample = ProjectConfig::sample(None, None);
        let yaml = serde_yaml::to_string(&sample).unwrap();
        ProjectConfig::parse(&yaml).unwrap();
    }

    #[test]
    pub fn test_extra_fields() {
        const YAML: &str = r#"---
version: 1.0.0
unknown: "a"
"#;
        ProjectConfig::parse(YAML).err().unwrap();
    }
}
