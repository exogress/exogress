#[macro_use]
extern crate shadow_clone;
#[macro_use]
extern crate tracing;
#[macro_use]
extern crate serde;

pub use connector::{Compression, ConnectTarget, Connector, INT_SUFFIX};
pub use error::Error;
pub use framed::{client_framed, server_framed};
pub use tunnel::{
    client_listener, server_connection, Conn, TunnelHello, TunnelHelloResponse, TunneledConnection,
};

mod connector;
mod error;
mod framed;
mod mixed_channel;
mod tunnel;

pub use mixed_channel::{to_async_rw, MixedChannel};
