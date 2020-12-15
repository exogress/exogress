use std::io;
use std::num::TryFromIntError;

use futures::channel::mpsc;

use crate::entities::{StringIdentifierParseError, Upstream};
use crate::tunnel::connector::ConnectTargetParseError;
use crate::tunnel::tunnel::Slot;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("io error: `{0}`")]
    Io(#[from] io::Error),

    #[error("could not convert from int: `{0}`")]
    TryFromInt(#[from] TryFromIntError),

    #[error("slot overflow")]
    SlotOverflow,

    #[error("unknown code {code}")]
    UnknownCode { code: u8 },

    #[error("internal send error")]
    SendError(#[from] mpsc::SendError),

    #[error("unknown slot {0}")]
    UnknownSlot(Slot),

    #[error("Accepted or Rejected received on established connection")]
    ConnectionHandshakeOnEstablishedConnection,

    #[error("data was sent on initiating connection")]
    CommandOnInitiatingConnection,

    #[error("could not parse string entity: {0}")]
    StringIdentifierParseError(#[from] StringIdentifierParseError),

    #[error("could not parse decode payload: {0}")]
    DecodeError(#[from] bincode::Error),

    #[error("could not resolve upstream: {upstream} hostname {host}")]
    UpstreamResolveError { upstream: Upstream, host: String },

    #[error("no hostname provided")]
    EmptyHost,

    #[error("connect target parse error: {0}")]
    ConnectTargetParseError(#[from] ConnectTargetParseError),
}
