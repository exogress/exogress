use core::fmt;
use std::str::FromStr;

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(deny_unknown_fields)]
pub struct Auth {
    pub providers: Vec<AuthDefinition>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(deny_unknown_fields)]
pub struct AuthDefinition {
    pub name: AuthProvider,
    pub acl: Vec<AclEntry>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(untagged)]
pub enum AclEntry {
    Allow {
        #[serde(rename = "allow")]
        identity: String,
    },
    Deny {
        #[serde(rename = "deny")]
        identity: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(deny_unknown_fields)]
pub enum AuthProvider {
    #[serde(rename = "google")]
    Google,

    #[serde(rename = "github")]
    Github,
}

impl FromStr for AuthProvider {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "google" => Ok(AuthProvider::Google),
            "github" => Ok(AuthProvider::Github),
            _ => Err(()),
        }
    }
}

impl fmt::Display for AuthProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthProvider::Google => write!(f, "google"),
            AuthProvider::Github => write!(f, "github"),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ClientHandler;

    #[test]
    fn test_acl_entry() {
        serde_yaml::from_str::<ClientHandler>(
            r#"
---
type: auth
priority: 1
providers:
  - name: google
    acl:
      - deny: "*@domain.tld"
      - allow: "*"
  - name: github
    acl: []
"#,
        )
        .unwrap();
    }
}
