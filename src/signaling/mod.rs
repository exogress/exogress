use crate::config_core::ClientConfig;
use crate::entities::{HealthCheckProbeName, InstanceId, Upstream};
use hashbrown::HashMap;
use http::StatusCode;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TunnelRequest {
    pub hostname: SmolStr,
    pub max_tunnels_count: u16,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TunnelRequestResponse {
    pub num_recipients: u16,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum SignalerHandshakeResponse {
    Ok { instance_id: InstanceId },
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