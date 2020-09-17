use crate::{AccountName, ConfigName, ProjectName};
use std::fmt;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, Hash, Eq, PartialEq)]
pub struct ConfigId {
    pub account_name: AccountName,
    pub project_name: ProjectName,
    pub config_name: ConfigName,
}

impl fmt::Display for ConfigId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}/{}/{}",
            self.account_name, self.project_name, self.config_name
        )
    }
}
