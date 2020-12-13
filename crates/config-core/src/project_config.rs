use hashbrown::HashSet;

use crate::auth::{AclEntry, AuthDefinition};
use crate::catch::RescueItem;
use crate::client_config::ClientMount;
use crate::config::default_rules;
use crate::path_segment::UrlPathSegmentOrQueryPart;
use crate::{Auth, AuthProvider, Config, ConfigVersion, Rule};
use crate::{ClientHandler, ClientHandlerVariant, StaticResponse, CURRENT_VERSION};
use exogress_entities::{HandlerName, MountPointName, StaticResponseName};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

#[derive(Serialize, Deserialize, Debug, Clone, Hash)]
#[serde(deny_unknown_fields)]
pub struct ProjectConfig {
    pub version: ConfigVersion,

    #[serde(
        rename = "mount-points",
        default,
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub mount_points: BTreeMap<MountPointName, ProjectMount>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rescue: Vec<RescueItem>,

    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        rename = "static-responses"
    )]
    pub static_responses: BTreeMap<StaticResponseName, StaticResponse>,
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
                    providers: vec![AuthDefinition {
                        name: AuthProvider::Google,
                        acl: vec![AclEntry::Allow {
                            identity: "*".into(),
                        }],
                    }],
                }),
                base_path: vec![],
                replace_base_path: vec![],
                rules: default_rules(),
                priority: 10,
                rescue: Default::default(),
            },
        );

        let static_responses = btreemap! {
            // response_name => btreemap!{
            //     "application/html".to_string() => StaticResponse {
            //         content: "<html><body>Not found. Generated by {{ this.config_name }} </body></html>"
            //             .to_string(),
            //         engine: None,
            //     }
            // }
        };

        let mount_points = btreemap! {
            mount_point_name => ProjectMount {
                handlers,
                rescue: Default::default(),
                static_responses,
            },
        };

        ProjectConfig {
            version: CURRENT_VERSION.clone(),
            mount_points,
            rescue: Default::default(),
            static_responses: Default::default(),
        }
    }
}

impl Default for ProjectConfig {
    fn default() -> Self {
        ProjectConfig {
            version: CURRENT_VERSION.clone(),
            mount_points: Default::default(),
            rescue: Default::default(),
            static_responses: Default::default(),
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
        if self.version != *CURRENT_VERSION {
            return Err(ProjectConfigError::UnsupportedVersion(self.version.clone()));
        }

        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash)]
#[serde(deny_unknown_fields)]
pub struct ProjectMount {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub handlers: BTreeMap<HandlerName, ProjectHandler>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rescue: Vec<RescueItem>,

    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        rename = "static-responses"
    )]
    pub static_responses: BTreeMap<StaticResponseName, StaticResponse>,
}

impl From<ProjectMount> for ClientMount {
    fn from(m: ProjectMount) -> Self {
        ClientMount {
            handlers: m.handlers.into_iter().map(|(k, v)| (k, v.into())).collect(),
            rescue: m.rescue,
            static_responses: m.static_responses,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(deny_unknown_fields, tag = "type")]
pub enum ProjectHandlerVariant {
    // #[serde(rename = "static_app")]
    // StaticApp(StaticApp),
    #[serde(rename = "auth")]
    Auth(Auth),
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash)]
// FIXME: report bug with enabling `deny_unknown_fields` and untagged and/or flatten enums
// #[serde(deny_unknown_fields)]
pub struct ProjectHandler {
    #[serde(flatten)]
    pub variant: ProjectHandlerVariant,

    #[serde(default, rename = "base-path")]
    pub base_path: Vec<UrlPathSegmentOrQueryPart>,

    #[serde(default, rename = "replace-base-path")]
    pub replace_base_path: Vec<UrlPathSegmentOrQueryPart>,

    #[serde(default = "default_rules")]
    pub rules: Vec<Rule>,

    pub priority: u16,

    #[serde(default)]
    pub rescue: Vec<RescueItem>,
}

impl From<ProjectHandler> for ClientHandler {
    fn from(f: ProjectHandler) -> Self {
        let v = match f.variant {
            ProjectHandlerVariant::Auth(auth) => ClientHandlerVariant::Auth(auth),
        };
        ClientHandler {
            variant: v,
            base_path: f.base_path,
            replace_base_path: f.replace_base_path,
            rules: f.rules,
            priority: f.priority,
            rescue: f.rescue,
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
        type: auth
        priority: 30
        providers:
          - name: github
            acl: []
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
        serde_yaml::from_str::<ProjectConfig>(YAML).unwrap();
    }

    #[test]
    pub fn test_sample() {
        let sample = ProjectConfig::sample(None, None);
        let yaml = serde_yaml::to_string(&sample).unwrap();
        let _: ProjectConfig = serde_yaml::from_str(&yaml).unwrap();
    }
}
