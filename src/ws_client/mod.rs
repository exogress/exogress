use rand::seq::IteratorRandom;
use rand::thread_rng;
use rustls::internal::pemfile::{certs, pkcs8_private_keys};
use std::io;
use std::io::Cursor;
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_rustls::rustls::ClientConfig;
use tokio_rustls::webpki::DNSNameRef;
use tokio_rustls::{client::TlsStream, TlsConnector};
use tokio_tungstenite::{client_async, WebSocketStream};
use tokio_util::either::Either;
use trust_dns_resolver::TokioAsyncResolver;
use tungstenite::http::{Request, Response};
use url::Url;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("bad schema")]
    BadSchema,

    #[error("no host")]
    NoHost,

    #[error("could not resolve ip")]
    NotResolved,

    #[error("url error: {}", _0)]
    UrlError(#[from] url::ParseError),

    #[error("resolve error: {}", _0)]
    ResolveError(#[from] trust_dns_resolver::error::ResolveError),

    #[error("io error: {}", _0)]
    IoError(#[from] io::Error),

    #[error("websocket (tungstenite) error: {}", _0)]
    WebsocketError(#[from] tungstenite::Error),

    #[error("TLS error: {}", _0)]
    TlsError(#[from] rustls::TLSError),
}

pub async fn connect_ws(
    req: Request<()>,
    resolver: TokioAsyncResolver,
    maybe_identity: Option<Vec<u8>>,
) -> Result<
    (
        WebSocketStream<Either<TlsStream<TcpStream>, TcpStream>>,
        Response<()>,
    ),
    Error,
> {
    let url = Url::parse(req.uri().to_string().as_str())?;
    let schema = url.scheme();
    let is_tls = if schema == "ws" {
        false
    } else if schema == "wss" {
        true
    } else {
        return Err(Error::BadSchema);
    };

    let host = url.host_str().ok_or(Error::NoHost)?;
    let ip = if let Ok(ip) = IpAddr::from_str(host) {
        ip
    } else {
        let ips = resolver.lookup_ip(host).await?;
        let mut rng = thread_rng();
        ips.iter().choose(&mut rng).ok_or(Error::NotResolved)?
    };

    let stream = TcpStream::connect((ip, url.port_or_known_default().unwrap())).await?;
    let _ = stream.set_nodelay(true);

    let stream = if is_tls {
        let mut config = ClientConfig::new();

        if let Some(mut identity_pem) = maybe_identity {
            let mut c = Cursor::new(&mut identity_pem);
            let pkey = pkcs8_private_keys(&mut c).expect("FIXME").pop().unwrap();
            let mut c = Cursor::new(&mut identity_pem);
            let certs = certs(&mut c).expect("FIXME");
            config.set_single_client_cert(certs, pkey)?;
        }
        config.root_store =
            rustls_native_certs::load_native_certs().expect("could not load platform certs");
        let config = TlsConnector::from(Arc::new(config));
        let host = url.host().unwrap().to_string();
        let dnsname = DNSNameRef::try_from_ascii_str(&host).unwrap();

        Either::Left(config.connect(dnsname, stream).await?)
    } else {
        Either::Right(stream)
    };

    Ok(client_async(req, stream).await?)
}
