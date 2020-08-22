use std::io;

use exogress_entities::{StringIdentifierParseError, TargetName, Upstream};
use futures::channel::{mpsc, oneshot};
use futures::future::BoxFuture;
use futures::task::Poll;
use futures::{task, FutureExt, SinkExt};
use hyper::Uri;
use smartstring::alias::*;
use url::Url;

use crate::tunnel::Conn;
use crate::TunneledConnection;
use core::fmt;
use std::str::FromStr;

/// Connect through established TCP tunnel
#[derive(Clone)]
pub struct Connector {
    req_tx: mpsc::Sender<(oneshot::Sender<Box<dyn Conn + 'static>>, ConnectTarget)>,
}

impl fmt::Debug for Connector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Connector")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectTarget {
    Upstream(Upstream),
    Internal(TargetName),
}

impl From<Upstream> for ConnectTarget {
    fn from(upstream: Upstream) -> Self {
        ConnectTarget::Upstream(upstream)
    }
}

impl From<TargetName> for ConnectTarget {
    fn from(target_name: TargetName) -> Self {
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

const UPSTREAM_SUFFIX: &str = ".upstream.exg";
const INT_SUFFIX: &str = ".int.exg";

impl ToString for ConnectTarget {
    fn to_string(&self) -> std::string::String {
        String::from(self.clone()).into()
    }
}

impl ConnectTarget {
    pub fn base_url(&self) -> Result<Url, url::ParseError> {
        Url::parse(format!("http://{}", self.to_string()).as_ref())
    }

    /// Chage hostname in the URL to connect target name
    pub fn update_url(&self, url: &mut Url) {
        url.set_host(Some(&self.to_string())).expect("FIXME");
    }
}

impl From<ConnectTarget> for String {
    fn from(target: ConnectTarget) -> Self {
        match target {
            ConnectTarget::Upstream(upstream) => String::from(upstream) + UPSTREAM_SUFFIX,
            ConnectTarget::Internal(int) => String::from(int) + INT_SUFFIX,
        }
    }
}

impl FromStr for ConnectTarget {
    type Err = ConnectTargetParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.ends_with(UPSTREAM_SUFFIX) {
            Ok(Upstream::from_str(s.strip_suffix(UPSTREAM_SUFFIX).unwrap())?.into())
        } else if s.ends_with(INT_SUFFIX) {
            Ok(ConnectTarget::Internal(TargetName::from_str(
                s.strip_suffix(INT_SUFFIX).unwrap(),
            )?))
        } else {
            Err(ConnectTargetParseError::BadKind(s.into()))
        }
    }
}

impl Connector {
    pub fn new(
        req_tx: mpsc::Sender<(oneshot::Sender<Box<dyn Conn + 'static>>, ConnectTarget)>,
    ) -> Self {
        Connector { req_tx }
    }

    pub fn get_connection(
        &self,
        connect_target: ConnectTarget,
    ) -> BoxFuture<'static, Result<TunneledConnection, crate::Error>> {
        let mut req_tx = self.req_tx.clone();

        async move {
            let (wait_tx, wait_rx) = oneshot::channel();

            req_tx.send((wait_tx, connect_target)).await.map_err(|_| {
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
fn extract_connect_target(uri: Uri) -> Result<ConnectTarget, crate::Error> {
    Ok(uri
        .host()
        .ok_or(crate::Error::EmptyHost)?
        .parse::<ConnectTarget>()?)
}

impl tower::Service<Uri> for Connector {
    type Response = TunneledConnection;
    type Error = crate::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, dst: Uri) -> Self::Future {
        let target_result: Result<ConnectTarget, crate::Error> = extract_connect_target(dst);
        match target_result {
            Ok(target) => self.get_connection(target),
            Err(e) => futures::future::ready(Err(e)).boxed(),
        }
    }
}
