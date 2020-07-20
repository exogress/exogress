#[macro_use]
extern crate serde;
#[macro_use]
extern crate tracing;

pub use config::{
    ConfigVersion, Revision, RootConfig as Config, Target, TargetVariant, UpstreamDefinition,
};
pub use proxy::Proxy;
pub use static_dir::StaticDir;

mod app;
mod cache;
mod config;
// mod mappings;
mod path;
mod path_segment;
mod proxy;
// mod redirect;
// mod rewrite;
mod client_config;
mod static_dir;

pub use client_config::ClientConfig;

pub const DEFAULT_CONFIG_FILE: &str = "Exofile";
