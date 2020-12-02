use crate::path::MatchingPath;
use crate::{Catch, StatusCode};
use exogress_entities::{ExceptionName, StaticResponseName};
use smol_str::SmolStr;
use std::collections::BTreeMap;

#[derive(Debug, Hash, Serialize, Deserialize, PartialEq, Clone)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub filter: Filter,
    // #[serde(default, skip_serializing_if = "Option::is_none")]
    // pub modify: Option<Modify>,
    #[serde(skip_serializing_if = "Action::is_none", default = "default_action")]
    pub action: Action,
}

fn default_action() -> Action {
    Action::None
}

#[derive(Debug, Hash, Serialize, Deserialize, PartialEq, Clone)]
#[serde(deny_unknown_fields)]
pub struct Filter {
    pub path: MatchingPath,
}

// #[derive(Debug, Hash, Eq, Serialize, Deserialize, PartialEq, Clone)]
// #[serde(deny_unknown_fields)]
// pub struct Modify {}

#[derive(Debug, Hash, Eq, Serialize, Deserialize, PartialEq, Clone)]
#[serde(deny_unknown_fields, tag = "kind")]
pub enum Action {
    /// process by the handler
    #[serde(rename = "invoke")]
    Invoke {
        #[serde(default)]
        catch: Catch,
    },

    /// stop rules processing and move on to the next handler
    #[serde(rename = "next-handler")]
    NextHandler,

    /// move on to the next rule. typically, combined with rewrite
    #[serde(rename = "none")]
    None,

    /// finish the whole handlers chain and move to finalizer
    #[serde(rename = "throw")]
    Throw {
        exception: ExceptionName,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        data: BTreeMap<SmolStr, SmolStr>,
    },

    /// finish the whole processing chain with the desired response
    #[serde(rename = "respond")]
    Respond {
        #[serde(rename = "static-response")]
        static_response_name: StaticResponseName,

        #[serde(rename = "status-code", default)]
        status_code: Option<StatusCode>,

        #[serde(default)]
        data: BTreeMap<SmolStr, SmolStr>,
    },
}

impl Default for Action {
    fn default() -> Self {
        Action::None
    }
}

impl Action {
    fn is_none(&self) -> bool {
        matches!(self, Action::None)
    }
}
