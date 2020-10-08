use crate::status_code::StatusCode;
use crate::StatusCodeRange;
use exogress_entities::{ExceptionName, StaticResponseName};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
#[serde(tag = "action", deny_unknown_fields)]
pub enum HandleAction {
    #[serde(rename = "static-response")]
    StaticResponse {
        #[serde(rename = "static-response-name")]
        static_response_name: StaticResponseName,

        #[serde(rename = "set-status-code", default)]
        set_status_code: Option<StatusCode>,

        #[serde(default)]
        data: Option<BTreeMap<String, String>>,
    },

    #[serde(rename = "throw")]
    Throw {
        #[serde(rename = "exception")]
        exception_name: ExceptionName,
    },
    #[serde(rename = "next-handler")]
    NextHandler,
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
pub struct StatusCodeRangeHandler {
    #[serde(rename = "status-codes-range")]
    status_codes_range: StatusCodeRange,

    #[serde(flatten)]
    handle_action: HandleAction,
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HandleWithStatusCode {
    #[serde(rename = "status-code")]
    status_code: StatusCode,

    #[serde(flatten)]
    handle_action: HandleAction,
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Default, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CatchActions {
    #[serde(default)]
    exceptions: BTreeMap<ExceptionName, HandleWithStatusCode>,
    #[serde(default, rename = "unhandled-exception")]
    unhandled_exception: Option<HandleWithStatusCode>,

    #[serde(default, rename = "status-codes")]
    status_codes: Vec<StatusCodeRangeHandler>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Default, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Catch {
    actions: CatchActions,
}
