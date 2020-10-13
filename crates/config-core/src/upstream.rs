use std::time::Duration;

#[derive(Serialize, Deserialize, Debug, Clone, Hash)]
#[serde(deny_unknown_fields)]
pub struct UpstreamDefinition {
    pub port: u16,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub health: Vec<Probe>,
}

impl UpstreamDefinition {
    pub fn on_default_host(port: u16) -> Self {
        UpstreamDefinition {
            port,
            host: None,
            health: vec![],
        }
    }

    pub fn get_host(&self) -> String {
        self.host.clone().unwrap_or_else(|| "127.0.0.1".to_string())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub enum ProbeKind {
    #[serde(rename = "liveness")]
    Liveness,
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Probe {
    pub kind: ProbeKind,
    pub target: ProbeTarget,
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProbeTarget {
    pub path: String,

    #[serde(with = "humantime_serde")]
    pub timeout: Duration,

    #[serde(with = "humantime_serde")]
    pub period: Duration,
}
