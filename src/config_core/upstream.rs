use crate::{
    config_core::{rule::HeaderMapWrapper, DurationWrapper, StatusCodeRange},
    entities::{HealthCheckProbeName, ProfileName},
};
use http::StatusCode;
use humantime::format_duration;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

use crate::config_core::rule::MethodWrapper;
use std::{
    collections::BTreeMap,
    hash::{Hash, Hasher},
    net::AddrParseError,
    num::ParseIntError,
    str::FromStr,
    time::Duration,
};

#[derive(thiserror::Error, Debug)]
pub enum UpstreamSocketAddrParseError {
    #[error("port error: {_0}")]
    PortError(#[from] ParseIntError),

    #[error("ip addr error: {_0}")]
    IpError(#[from] AddrParseError),

    #[error("malformed addr")]
    Malformed,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
// #[schemars(deny_unknown_fields)]
pub struct UpstreamSocketAddr {
    pub port: u16,
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
        Ok(if let Some(stripped) = s.strip_prefix(':') {
            UpstreamSocketAddr {
                port: stripped.parse()?,
                host: None,
            }
        } else if s.contains(':') {
            let mut parts: Vec<_> = s.split(':').collect();
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

#[derive(Serialize, Deserialize, Debug, Clone, Hash, JsonSchema)]
// #[schemars(deny_unknown_fields)]
pub struct UpstreamDefinition {
    #[serde(flatten)]
    pub addr: UpstreamSocketAddr,

    #[serde(rename = "health-checks", default)]
    pub health_checks: BTreeMap<HealthCheckProbeName, Probe>,

    #[serde(default)]
    pub profiles: Option<Vec<ProfileName>>,
}

impl UpstreamDefinition {
    pub fn on_default_host(port: u16) -> Self {
        UpstreamDefinition {
            addr: UpstreamSocketAddr { port, host: None },
            health_checks: BTreeMap::new(),
            profiles: None,
        }
    }

    pub fn get_host(&self) -> String {
        self.addr
            .host
            .clone()
            .unwrap_or_else(|| "127.0.0.1".to_string())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq, JsonSchema)]
#[serde(tag = "kind")]
// #[schemars(deny_unknown_fields)]
pub enum ProbeDetails {
    #[serde(rename = "liveness")]
    Liveness,
}

fn default_status_code_range() -> StatusCodeRange {
    StatusCodeRange::Single(StatusCode::OK)
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq, JsonSchema)]
// #[schemars(deny_unknown_fields)]
pub struct Probe {
    #[serde(flatten)]
    pub details: ProbeDetails,
    pub path: SmolStr,
    pub timeout: DurationWrapper,
    pub period: DurationWrapper,

    #[serde(default)]
    pub headers: HeaderMapWrapper,

    #[serde(default = "MethodWrapper::default")]
    pub method: MethodWrapper,

    #[serde(rename = "expected-status-code", default = "default_status_code_range")]
    pub expected_status_code: StatusCodeRange,
}

#[derive(thiserror::Error, Debug)]
pub enum ProbeError {
    #[error("bad path provided")]
    BadPath,

    #[error("timeout is below threshold of {}", format_duration(*threshold))]
    TimeoutBelowThreshold { threshold: Duration },

    #[error("period is below threshold of {}", format_duration(*threshold))]
    PeriodBelowThreshold { threshold: Duration },
}

impl Probe {
    const PERIOD_THRESHOLD: Duration = Duration::from_secs(1);
    const TIMEOUT_THRESHOLD: Duration = Duration::from_secs(1);

    pub fn validate(&self) -> Result<(), ProbeError> {
        if self.period.0 < Probe::PERIOD_THRESHOLD {
            return Err(ProbeError::PeriodBelowThreshold {
                threshold: Probe::PERIOD_THRESHOLD,
            });
        }
        if self.timeout.0 < Probe::TIMEOUT_THRESHOLD {
            return Err(ProbeError::TimeoutBelowThreshold {
                threshold: Probe::TIMEOUT_THRESHOLD,
            });
        }
        Ok(())
    }
}
