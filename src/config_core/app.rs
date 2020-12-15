use semver::Version;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(deny_unknown_fields, tag = "app")]
pub enum StaticApp {
    #[serde(rename = "swagger-ui")]
    SwaggerUi(SwaggerUi),
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct SwaggerUi {
    version: Version,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn test_deserialize() {
        assert_eq!(
            StaticApp::SwaggerUi(SwaggerUi {
                version: "0.5.0".parse().unwrap()
            }),
            serde_yaml::from_str(
                r#"
---
app: swagger-ui
version: "0.5.0"
"#
            )
            .unwrap()
        );
    }
}
