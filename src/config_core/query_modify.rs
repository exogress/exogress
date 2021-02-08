use crate::config_core::UrlPathSegment;
use smol_str::SmolStr;
use std::collections::{BTreeMap, BTreeSet};

pub enum QueryModify {
    Modify {
        remove: BTreeSet<UrlPathSegment>,
        insert: BTreeMap<UrlPathSegment, SmolStr>,
    },
    Set {
        params: Vec(SmolStr, SmolStr),
    },
}
