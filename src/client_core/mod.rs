mod client;
mod health;
mod internal_server;
mod signal_client;
mod tunnel;

pub use client::{Client, ClientBuilder, DEFAULT_CLOUD_ENDPOINT};
use dashmap::DashMap;
use futures::channel::oneshot;
use hashbrown::HashMap;
use smol_str::SmolStr;
use std::sync::Arc;

type TunnelsStorage = Arc<DashMap<SmolStr, HashMap<u16, oneshot::Sender<()>>>>;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
