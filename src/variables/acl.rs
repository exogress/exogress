use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(transparent)]
pub struct Acl(Vec<AclEntry>);

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(untagged)]
pub enum AclEntry {
    Allow {
        #[serde(rename = "allow")]
        identity: SmolStr,
    },
    Deny {
        #[serde(rename = "deny")]
        identity: SmolStr,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acl_entry() {
        serde_yaml::from_str::<Acl>(
            r#"
---
- deny: "*@domain.tld"
- allow: "*"
"#,
        )
        .unwrap();
    }
}
