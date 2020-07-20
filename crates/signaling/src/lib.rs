#[macro_use]
extern crate serde;

use smartstring::alias::String;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TunnelRequest {
    pub hostname: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TunnelRequestResponse {
    pub num_recipients: u16,
}
