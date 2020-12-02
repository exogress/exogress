pub use bytes;
pub use rand;
pub use serde;
pub use tracing;
pub use ulid::{self, Ulid, ULID_LEN};

pub use config_id::ConfigId;
pub use label_value::LabelValue;
pub use smol_str::SmolStr;
pub use string_macros::{
    StringIdentifierError, StringIdentifierParseError, MAX_STRING_IDENTIFIER_LENGTH,
    MIN_STRING_IDENTIFIER_LENGTH,
};
pub use ulid_macro::UlidIdentifierParseError;

mod config_id;
mod label_value;
mod string_macros;
mod ulid_macro;

ulid_type!(InstanceId);
ulid_type!(EnvironmentId);
ulid_type!(AccessKeyId);
ulid_type!(TunnelId);
ulid_type!(AccountUniqueId);

string_type!(ProjectName, crate::SmolStr);
string_type!(AccountName, crate::SmolStr);

string_type!(Upstream, crate::SmolStr); //Not sure SmolStr vs SmolStr

string_type!(RateLimiterName, crate::SmolStr);
string_type!(MountPointName, crate::SmolStr);
string_type!(HandlerName, crate::SmolStr);
string_type!(ConfigName, crate::SmolStr);
string_type!(LabelName, crate::SmolStr);
string_type!(StaticResponseName, crate::SmolStr);
string_type!(ExceptionName, crate::SmolStr);
