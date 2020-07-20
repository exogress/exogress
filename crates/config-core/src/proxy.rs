use serde::Deserialize;

use exogress_entities::Upstream;

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(deny_unknown_fields)]
pub struct Proxy {
    pub upstream: Upstream,
}

// #[derive(Serialize, Deserialize, Debug, Clone, Copy)]
// #[serde(deny_unknown_fields)]
// pub enum Method {
//     #[serde(rename = "GET")]
//     Get,
//     #[serde(rename = "POST")]
//     Post,
//     #[serde(rename = "PUT")]
//     Put,
//     #[serde(rename = "DELETE")]
//     Delete,
//     #[serde(rename = "HEAD")]
//     Head,
//     #[serde(rename = "OPTIONS")]
//     Options,
//     #[serde(rename = "CONNECT")]
//     Connect,
//     #[serde(rename = "PATCH")]
//     Patch,
//     #[serde(rename = "TRACE")]
//     Trace,
// }
