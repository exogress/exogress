use hashbrown::HashSet;

use crate::path_segment::UrlPathSegmentOrQueryPart;
use crate::{Auth, AuthProvider, Config, ConfigVersion};
use exogress_entities::{HandlerName, MountPointName};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

#[derive(Serialize, Deserialize, Debug, Clone, Hash)]
#[serde(deny_unknown_fields)]
pub struct ProjectConfig {
    pub version: ConfigVersion,
    pub mount_points: BTreeMap<MountPointName, ProjectMount>,
}

impl ProjectConfig {
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
                    provider: AuthProvider::Google,
                }),
                base_path: vec![],
                priority: 10,
            },
        );

        let mut mount_points = BTreeMap::new();
        mount_points.insert(mount_point_name, ProjectMount { handlers });

        ProjectConfig {
            version: "0.0.1".parse().unwrap(),
            mount_points,
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
        if self.version != "0.0.1".parse().unwrap() {
            return Err(ProjectConfigError::UnsupportedVersion(self.version.clone()));
        }

        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash)]
#[serde(deny_unknown_fields)]
pub struct ProjectMount {
    pub handlers: BTreeMap<HandlerName, ProjectHandler>,
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
// FIXME: report bug with enabling `deny_unknown_fields`
// #[serde(deny_unknown_fields)]
pub struct ProjectHandler {
    #[serde(flatten)]
    pub variant: ProjectHandlerVariant,

    #[serde(default)]
    pub base_path: Vec<UrlPathSegmentOrQueryPart>,

    pub priority: u16,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn test_parsing() {
        const YAML: &str = r#"---
version: 0.0.1
mount_points:
  mount_point:
    handlers:
      main:
        type: auth
        priority: 30
        provider: google
"#;
        serde_yaml::from_str::<ProjectConfig>(YAML).unwrap();
    }

    #[test]
    pub fn test_sample() {
        ProjectConfig::sample(None, None);
    }
}
