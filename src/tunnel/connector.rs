use std::io;

use crate::common_utils::uri_ext::UriExt;
use crate::entities::{HandlerName, StringIdentifierParseError, Upstream};
use crate::tunnel::Conn;
use crate::tunnel::TunneledConnection;
use core::fmt;
use futures::channel::{mpsc, oneshot};
use futures::future::BoxFuture;
use futures::task::Poll;
use futures::{task, FutureExt, SinkExt};
use hyper::service::Service;
use hyper::Uri;
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use ulid::Ulid;
use url::Url;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Compression {
    Plain,
    Zstd,
}

/// Connect through established TCP tunnel
#[derive(Clone)]
pub struct Connector {
    req_tx: mpsc::Sender<ConnectorRequest>,
    ulid: Ulid,
}

impl Hash for Connector {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u128(self.ulid.0)
    }
}

impl PartialEq for Connector {
    fn eq(&self, other: &Self) -> bool {
        self.ulid.eq(&other.ulid)
    }
}

impl Eq for Connector {}

impl fmt::Debug for Connector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Connector")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectTarget {
    Upstream(Upstream),
    Internal(HandlerName),
}

impl From<Upstream> for ConnectTarget {
    fn from(upstream: Upstream) -> Self {
        ConnectTarget::Upstream(upstream)
    }
}

impl From<HandlerName> for ConnectTarget {
    fn from(target_name: HandlerName) -> Self {
        ConnectTarget::Internal(target_name)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ConnectTargetParseError {
    #[error("bad connect target kind: `{0}`")]
    BadKind(String),

    #[error("bad upstream: {0}")]
    BadUpstream(#[from] StringIdentifierParseError),
}

pub const UPSTREAM_SUFFIX: &str = ".upstream.exg";
pub const INT_SUFFIX: &str = ".int.exg";

impl ConnectTarget {
    pub fn hostname(&self) -> String {
        match self {
            ConnectTarget::Upstream(upstream) => String::from(upstream.clone()) + UPSTREAM_SUFFIX,
            ConnectTarget::Internal(int) => String::from(int.clone()) + INT_SUFFIX,
        }
    }

    pub fn base_url(&self) -> Result<Url, url::ParseError> {
        Url::parse(format!("http://{}", self.hostname()).as_ref())
    }

    /// Change hostname in the URL to connect target name
    pub fn update_url(&self, uri: &mut http::uri::Uri) {
        uri.unset_port();
        uri.set_hostname(self.hostname().as_str());
    }

    pub fn with_path(&self, path: &str) -> Result<Url, url::ParseError> {
        Url::parse(&format!("http://{}{}", self.hostname(), path))
    }
}

impl FromStr for ConnectTarget {
    type Err = ConnectTargetParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.ends_with(UPSTREAM_SUFFIX) {
            Ok(Upstream::from_str(s.strip_suffix(UPSTREAM_SUFFIX).unwrap())?.into())
        } else if s.ends_with(INT_SUFFIX) {
            Ok(ConnectTarget::Internal(HandlerName::from_str(
                s.strip_suffix(INT_SUFFIX).unwrap(),
            )?))
        } else {
            Err(ConnectTargetParseError::BadKind(s.into()))
        }
    }
}

pub struct ConnectorRequest {
    pub tx: oneshot::Sender<Box<dyn Conn + 'static>>,
    pub target: ConnectTarget,
    pub compression: Compression,
}

impl Connector {
    pub fn new(req_tx: mpsc::Sender<ConnectorRequest>) -> Self {
        Connector {
            req_tx,
            ulid: Ulid::new(),
        }
    }

    pub fn retrieve_connection(
        &self,
        connect_target: ConnectTarget,
        compression: Compression,
    ) -> BoxFuture<'static, Result<TunneledConnection, crate::tunnel::Error>> {
        let mut req_tx = self.req_tx.clone();

        async move {
            let (wait_tx, wait_rx) = oneshot::channel();

            req_tx
                .send(ConnectorRequest {
                    tx: wait_tx,
                    target: connect_target,
                    compression,
                })
                .await
                .map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::Other,
                        "tunnel already closed: could not invoke request for new connection",
                    )
                })?;

            let c = wait_rx.await.map_err(|_| {
                io::Error::new(
                    io::ErrorKind::Other,
                    "tunnel already closed: unable to wait for new connection",
                )
            })?;

            Ok(TunneledConnection::new(c))
        }
        .boxed()
    }
}

#[inline]
fn extract_connect_target(uri: Uri) -> Result<ConnectTarget, crate::tunnel::Error> {
    Ok(uri
        .host()
        .ok_or(crate::tunnel::Error::EmptyHost)?
        .parse::<ConnectTarget>()?)
}

impl Service<Uri> for Connector {
    type Response = TunneledConnection;
    type Error = crate::tunnel::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    // TODO: implement poll_ready?
    fn poll_ready(&mut self, _cx: &mut task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, dst: Uri) -> Self::Future {
        let target_result: Result<ConnectTarget, crate::tunnel::Error> =
            extract_connect_target(dst);
        match target_result {
            Ok(target) => self.retrieve_connection(target, Compression::Zstd),
            Err(e) => futures::future::ready(Err(e)).boxed(),
        }
    }
}
