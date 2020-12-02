use crate::status_code::StatusCode;
use crate::StatusCodeRange;
use exogress_entities::{ExceptionName, StaticResponseName};
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
#[serde(tag = "action", deny_unknown_fields)]
pub enum CatchAction {
    #[serde(rename = "static-response")]
    StaticResponse {
        #[serde(rename = "static-response-name")]
        static_response_name: StaticResponseName,

        #[serde(rename = "status-code", default)]
        status_code: Option<StatusCode>,

        #[serde(default)]
        data: BTreeMap<SmolStr, SmolStr>,
    },

    #[serde(rename = "throw")]
    Throw {
        #[serde(rename = "exception")]
        exception_name: ExceptionName,

        #[serde(default)]
        data: BTreeMap<SmolStr, SmolStr>,
    },
    #[serde(rename = "next-handler")]
    NextHandler,
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
pub struct StatusCodeRangeHandler {
    #[serde(rename = "status-codes-range")]
    pub status_codes_range: StatusCodeRange,

    #[serde(flatten)]
    pub catch: CatchAction,
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Default, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CatchActions {
    #[serde(default)]
    pub exceptions: BTreeMap<ExceptionName, CatchAction>,
    #[serde(default, rename = "unhandled-exception")]
    pub unhandled_exception: Option<CatchAction>,

    #[serde(default, rename = "status-codes")]
    pub status_codes: Vec<StatusCodeRangeHandler>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Default, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Catch {
    pub actions: CatchActions,
}
