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