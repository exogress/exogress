pub use bytes;
pub use serde;
pub use tracing;
pub use ulid::{self, Ulid, ULID_LEN};

pub use config_id::ConfigId;
use core::fmt;
pub use label_value::LabelValue;
use never::Never;
use serde::{
    de, de::DeserializeOwned, ser::Error, Deserialize, Deserializer, Serialize, Serializer,
};
pub use smol_str::SmolStr;
use std::str::FromStr;
pub use string_macros::{
    StringIdentifierError, StringIdentifierParseError, MAX_STRING_IDENTIFIER_LENGTH,
    MIN_STRING_IDENTIFIER_LENGTH,
};
pub use ulid_macro::UlidIdentifierParseError;

mod config_id;
mod label_value;
#[macro_use]
mod string_macros;
#[macro_use]
mod ulid_macro;

ulid_type!(InstanceId);
ulid_type!(EnvironmentId);
ulid_type!(AccessKeyId);
ulid_type!(TunnelId);
ulid_type!(AccountUniqueId);

string_type!(ProjectName);
string_type!(AccountName);

string_type!(Upstream);

string_type!(RateLimiterName);
string_type!(MountPointName);
string_type!(HandlerName);
string_type!(ConfigName);
string_type!(LabelName);
string_type!(StaticResponseName);
string_type!(ExceptionSegment);
string_type!(HealthCheckProbeName);
string_type!(ParameterName);
string_type!(ProfileName);

pub trait SharedEntity:
    DeserializeOwned
    + Serialize
    + core::fmt::Debug
    + Clone
    + Eq
    + PartialEq
    + std::hash::Hash
    + core::fmt::Display
    + FromStr
{
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct NonExistingSharedEntity(Never);

impl SharedEntity for NonExistingSharedEntity {}

impl fmt::Display for NonExistingSharedEntity {
    fn fmt(&self, _f: &mut fmt::Formatter) -> fmt::Result {
        Err(fmt::Error)
    }
}

impl Serialize for NonExistingSharedEntity {
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        Err(S::Error::custom(
            "impossible to serialize - non existing entity",
        ))
    }
}

impl<'de> Deserialize<'de> for NonExistingSharedEntity {
    fn deserialize<D>(_deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        Err(de::Error::custom(
            "impossible to deserialize - non existing entity",
        ))
    }
}

impl FromStr for NonExistingSharedEntity {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Err(())
    }
}

impl SharedEntity for StaticResponseName {}
