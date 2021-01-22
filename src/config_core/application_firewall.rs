use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(deny_unknown_fields)]
pub struct ApplicationFirewall {
    #[serde(rename = "uri-xss")]
    pub uri_xss: bool,
    #[serde(rename = "uri-sqli")]
    pub uri_sqli: bool,
}

impl Default for ApplicationFirewall {
    fn default() -> Self {
        Self {
            uri_xss: true,
            uri_sqli: true,
        }
    }
}
