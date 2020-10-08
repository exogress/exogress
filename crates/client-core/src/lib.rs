#[macro_use]
extern crate derive_builder;
#[macro_use]
extern crate tracing;
#[macro_use]
extern crate serde;

mod client;
mod internal_server;
mod signal_client;
mod tunnel;

pub use client::{Client, ClientBuilder, DEFAULT_CLOUD_ENDPOINT};
use dashmap::DashMap;
use futures::channel::oneshot;
use hashbrown::HashMap;
use std::sync::Arc;

type TunnelsStorage = Arc<DashMap<String, HashMap<u16, oneshot::Sender<()>>>>;
