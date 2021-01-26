use std::io;

use std::net::SocketAddr;

use crate::config_core::ClientConfig;
use crate::entities::{AccessKeyId, AccountName, InstanceId, ProfileName, ProjectName, SmolStr};
use crate::tunnel::{
    client_framed, client_listener, MixedChannel, TunnelHello, TunnelHelloResponse, ALPN_PROTOCOL,
};
use core::time::Duration;
use futures::channel::mpsc;
use parking_lot::RwLock;
use rand::seq::IteratorRandom;
use rand::thread_rng;
use rustls::ClientConfig as RustlsClientConfig;
use rw_stream_sink::RwStreamSink;
use std::convert::TryInto;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::webpki::DNSNameRef;
use tokio_rustls::{rustls, TlsConnector};
use tracing::{error, info};
use trust_dns_resolver::error::ResolveError;
use trust_dns_resolver::TokioAsyncResolver;
use warp::hyper::client::conn;
use webpki::InvalidDNSNameError;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("TLS error: `{0}`")]
    Tls(#[from] rustls::TLSError),

    #[error("io error: `{0}`")]
    Io(#[from] io::Error),

    #[error("hyper error: `{0}`")]
    Hyper(#[from] hyper::Error),

    #[error("tunnel error: `{0}`")]
    Tunnel(#[from] crate::tunnel::Error),

    #[error("tunnel rejected with message: `{0}`")]
    Rejected(String),

    #[error("tunnel establish timeout")]
    EstablishTimeout,

    #[error("resolve error: `{_0}`")]
    ResolveError(#[from] ResolveError),

    #[error("no addresses resolved")]
    NothingResolved,

    #[error("invalid DNS name: `{_0}`")]
    BadDnsName(#[from] InvalidDNSNameError),

    #[error("bad http status code: `{_0}`")]
    BadHttpStatus(http::StatusCode),
}

#[allow(clippy::too_many_arguments)]
/// Returns true if tunnel creation should be retried, false otherwise
pub async fn spawn(
    client_config: Arc<RwLock<ClientConfig>>,
    account_name: AccountName,
    project_name: ProjectName,
    instance_id: InstanceId,
    access_key_id: AccessKeyId,
    secret_access_key: SmolStr,
    gw_hostname: SmolStr,
    gw_port: u16,
    active_profile: &Option<ProfileName>,
    internal_server_connector: mpsc::Sender<RwStreamSink<MixedChannel>>,
    resolver: TokioAsyncResolver,
) -> Result<bool, Error> {
    let (tunnel_id, stream) = tokio::time::timeout(Duration::from_secs(5), async {
        info!("connecting tunnel to server");
        let gw_addrs = resolver.lookup_ip(gw_hostname.to_string()).await?;
        let gw_addr = gw_addrs
            .iter()
            .choose(&mut thread_rng())
            .ok_or(Error::NothingResolved)?;

        let socket = TcpStream::connect(SocketAddr::new(gw_addr, gw_port)).await?;
        let _ = socket.set_nodelay(true);
        let mut config = RustlsClientConfig::new();
        config.alpn_protocols = vec![ALPN_PROTOCOL.to_vec(), b"http/1.1".to_vec()];
        config.root_store =
            rustls_native_certs::load_native_certs().expect("could not load platform certs");
        let config = TlsConnector::from(Arc::new(config));
        let dnsname = DNSNameRef::try_from_ascii_str(&gw_hostname)?;

        info!("connect to {}, addr={}", gw_hostname, gw_addr);

        let tls_stream = config.connect(dnsname, socket).await?;

        let (mut send_request, http_connection) = conn::Builder::new()
            .http2_only(false)
            .handshake(tls_stream)
            .await?;

        tokio::spawn(http_connection);

        let req = http::Request::builder()
            .uri(format!("https://{}/exotun", gw_hostname))
            .header("upgrade", "exotun")
            .header("connection", "upgrade")
            .body(hyper::Body::empty())
            .unwrap();

        let mut res = send_request.send_request(req).await?;

        if res.status() != http::StatusCode::SWITCHING_PROTOCOLS {
            return Err(Error::BadHttpStatus(res.status()));
        }

        let mut stream = hyper::upgrade::on(&mut res).await?;

        let hello = TunnelHello {
            config_name: client_config.read().name.clone(),
            account_name,
            project_name,
            instance_id,
            access_key_id,
            secret_access_key,
        };

        let encoded_hello: Vec<u8> = bincode::serialize(&hello).unwrap();
        stream
            .write_u16(encoded_hello.len().try_into().unwrap())
            .await?;
        stream.write_all(&encoded_hello).await?;

        let resp_len = stream.read_u16().await?.into();
        let mut tunnel_hello_response = vec![0u8; resp_len];
        stream.read_exact(&mut tunnel_hello_response).await?;
        let hello_response = bincode::deserialize::<TunnelHelloResponse>(&tunnel_hello_response)
            .map_err(crate::tunnel::Error::DecodeError)?;

        match hello_response {
            TunnelHelloResponse::Ok { tunnel_id } => Ok((tunnel_id, stream)),
            TunnelHelloResponse::Err { msg } => Err(Error::Rejected(msg)),
        }
    })
    .await
    .map_err(|_| Error::EstablishTimeout)??;

    info!("tunnel established. tunnel_id = {}", tunnel_id);

    let r = client_listener(
        client_framed(stream),
        client_config,
        internal_server_connector,
        active_profile,
        resolver.clone(),
    )
    .await?;

    Ok(r)
}
