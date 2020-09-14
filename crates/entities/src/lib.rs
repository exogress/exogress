pub use bytes;
pub use rand;
pub use serde;
pub use smartstring::alias::String;
pub use tracing;
pub use ulid::{self, Ulid, ULID_LEN};

pub use label_value::LabelValue;
pub use string_macros::{
    StringIdentifierError, StringIdentifierParseError, MAX_STRING_IDENTIFIER_LENGTH,
    MIN_STRING_IDENTIFIER_LENGTH,
};
pub use ulid_macro::UlidIdentifierParseError;

mod label_value;
mod string_macros;
mod ulid_macro;

ulid_type!(InstanceId);
ulid_type!(EnvironmentId);
ulid_type!(AccessKeyId);
ulid_type!(TunnelId);

string_type!(RateLimiterName);
string_type!(ProjectName);
string_type!(AccountName);
string_type!(MountPointName);
string_type!(HandlerName);
string_type!(ConfigName);
string_type!(Upstream);
string_type!(LabelName);
