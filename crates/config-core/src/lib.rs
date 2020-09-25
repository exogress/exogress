#[macro_use]
extern crate serde;

pub use auth::{AclEntry, Auth, AuthDefinition, AuthProvider};
pub use client_config::{ClientConfig, ClientConfigRevision, ClientHandler, ClientHandlerVariant};
pub use config::Config;
use lazy_static::lazy_static;
pub use project_config::ProjectConfig;
pub use proxy::Proxy;
use semver::Version;
pub use static_dir::StaticDir;
pub use upstream::{Probe, UpstreamDefinition};
pub use version::ConfigVersion;

mod app;
mod auth;
mod cache;
mod client_config;
mod config;
mod path;
mod path_segment;
mod project_config;
mod proxy;
mod static_dir;
mod upstream;
mod version;

pub const DEFAULT_CONFIG_FILE: &str = "Exofile";

lazy_static! {
    pub static ref CURRENT_VERSION: ConfigVersion = ConfigVersion(Version::new(0, 0, 1));
}
