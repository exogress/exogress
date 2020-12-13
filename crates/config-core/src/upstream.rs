use crate::{HttpHeaders, StatusCodeRange};
use exogress_entities::HealthCheckProbeName;
use http::Method;
use smol_str::SmolStr;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::net::AddrParseError;
use std::num::ParseIntError;
use std::str::FromStr;
use std::time::Duration;

#[derive(thiserror::Error, Debug)]
pub enum UpstreamSocketAddrParseError {
    #[error("port error: {_0}")]
    PortError(#[from] ParseIntError),

    #[error("ip addr error: {_0}")]
    IpError(#[from] AddrParseError),

    #[error("malformed addr")]
    Malformed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamSocketAddr {
    pub port: u16,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
}

impl Hash for UpstreamSocketAddr {
    fn hash<H: Hasher>(&self, _state: &mut H) {
        // Hash on config structures is used to count checksum
        // We don't consider socket addr here, because it may be redefined on per-instance basis (under the same config name)
    }
}

impl FromStr for UpstreamSocketAddr {
    type Err = UpstreamSocketAddrParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(if s.starts_with(":") {
            UpstreamSocketAddr {
                port: s[1..].parse()?,
                host: None,
            }
        } else if s.contains(':') {
            let mut parts: Vec<_> = s.split(":").collect();
            if parts.len() != 2 {
                return Err(UpstreamSocketAddrParseError::Malformed);
            }
            let port = parts.pop().unwrap().parse()?;
            let addr = parts.pop().unwrap().to_string();

            UpstreamSocketAddr {
                port,
                host: Some(addr),
            }
        } else {
            return Err(UpstreamSocketAddrParseError::Malformed);
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash)]
#[serde(deny_unknown_fields)]
pub struct UpstreamDefinition {
    #[serde(flatten)]
    pub addr: UpstreamSocketAddr,

    #[serde(
        rename = "health-checks",
        default,
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub health_checks: BTreeMap<HealthCheckProbeName, Probe>,
}

impl UpstreamDefinition {
    pub fn on_default_host(port: u16) -> Self {
        UpstreamDefinition {
            addr: UpstreamSocketAddr { port, host: None },
            health_checks: BTreeMap::new(),
        }
    }

    pub fn get_host(&self) -> String {
        self.addr
            .host
            .clone()
            .unwrap_or_else(|| "127.0.0.1".to_string())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
#[serde(deny_unknown_fields, tag = "kind")]
pub enum ProbeDetails {
    #[serde(rename = "liveness")]
    Liveness,
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
pub struct Probe {
    #[serde(flatten)]
    pub details: ProbeDetails,
    pub target: ProbeTarget,

    #[serde(flatten)]
    pub headers: HttpHeaders,

    #[serde(with = "http_serde::method")]
    pub method: Method,

    #[serde(rename = "expected-status-code")]
    pub expected_status_code: StatusCodeRange,
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProbeTarget {
    pub path: SmolStr,

    #[serde(with = "humantime_serde")]
    pub timeout: Duration,

    #[serde(with = "humantime_serde")]
    pub period: Duration,
}
