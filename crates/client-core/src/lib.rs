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
use futures::channel::oneshot;
use hashbrown::HashMap;
use parking_lot::Mutex;
use std::sync::Arc;

type TunnelsStorage =
    Arc<Mutex<HashMap<smartstring::alias::String, HashMap<u16, oneshot::Sender<()>>>>>;
