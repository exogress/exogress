use crate::{
    config_core::ClientConfig,
    entities::{url_prefix::MountPointBaseUrl, HealthCheckProbeName, InstanceId, Upstream},
};
use core::fmt;
use hashbrown::HashMap;
use http::StatusCode;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum WsCloudToInstanceMessage {
    TunnelRequest(TunnelRequest),
    ConfigUpdateResult(ConfigUpdateResult),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TunnelRequest {
    pub hostname: SmolStr,
    pub max_tunnels_count: u16,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum ConfigUpdateResult {
    Error { msg: String },
    Ok { base_urls: Vec<MountPointBaseUrl> },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TunnelRequestResponse {
    pub num_recipients: u16,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum SignalerHandshakeResponse {
    Ok {
        instance_id: InstanceId,
        base_urls: Vec<MountPointBaseUrl>,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InstanceConfigMessage {
    pub config: ClientConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum WsInstanceToCloudMessage {
    InstanceConfig(InstanceConfigMessage),
    HealthState(HashMap<Upstream, HashMap<HealthCheckProbeName, ProbeHealthStatus>>),
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum UnhealthyReason {
    #[serde(rename = "timeout")]
    Timeout,
    #[serde(rename = "bad-status")]
    BadStatus {
        #[serde(with = "http_serde::status_code")]
        status: StatusCode,
    },
    #[serde(rename = "request-error")]
    RequestError { err: String },
}

impl fmt::Display for UnhealthyReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UnhealthyReason::Timeout => {
                write!(f, "timeout")
            }
            UnhealthyReason::BadStatus { status } => {
                write!(f, "bad status {}", status)
            }
            UnhealthyReason::RequestError { err } => {
                write!(f, "request error: `{}`", err)
            }
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ProbeHealthStatus {
    #[serde(rename = "healthy")]
    Healthy,
    #[serde(rename = "unhealthy")]
    Unhealthy { reason: UnhealthyReason },
    #[serde(rename = "unknown")]
    Unknown,
}

impl Default for ProbeHealthStatus {
    fn default() -> Self {
        Self::Unknown
    }
}
