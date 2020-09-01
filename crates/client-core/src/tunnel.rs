use smartstring::alias::String;
use std::io;

use std::net::SocketAddr;

use exogress_config_core::ClientConfig;
use exogress_entities::InstanceId;
use exogress_tunnel::{client_framed, client_listener, MixedChannel, TunnelHello};
use futures::channel::mpsc;
use parking_lot::RwLock;
use rand::rngs::SmallRng;
use rand::seq::IteratorRandom;
use rustls::ClientConfig as RustlsClientConfig;
use rw_stream_sink::RwStreamSink;
use std::convert::TryInto;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio_rustls::webpki::DNSNameRef;
use tokio_rustls::{rustls, TlsConnector};
use trust_dns_resolver::TokioAsyncResolver;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("TLS error: `{0}`")]
    Tls(#[from] rustls::TLSError),

    #[error("io error: `{0}`")]
    Io(#[from] io::Error),

    #[error("tunnel error: `{0}`")]
    Tunnel(#[from] exogress_tunnel::Error),
}

pub async fn spawn(
    client_config: Arc<RwLock<ClientConfig>>,
    instance_id: InstanceId,
    gw_hostname: String,
    internal_server_connector: mpsc::Sender<RwStreamSink<MixedChannel>>,
    resolver: TokioAsyncResolver,
    small_rng: &mut SmallRng,
) -> Result<(), Error> {
    info!("connecting tunnel to server");
    let gw_addrs = resolver.lookup_ip(gw_hostname.to_string()).await.unwrap();
    let gw_addr = gw_addrs.iter().choose(small_rng).unwrap();

    let socket = TcpStream::connect(SocketAddr::new(gw_addr, 10714)).await?;
    let _ = socket.set_nodelay(true);
    let mut config = RustlsClientConfig::new();
    config.root_store =
        rustls_native_certs::load_native_certs().expect("could not load platform certs");
    let config = TlsConnector::from(Arc::new(config));
    let dnsname = DNSNameRef::try_from_ascii_str(&gw_hostname).unwrap();

    info!("connect to {}, addr={}", gw_hostname, gw_addr);

    let mut stream = config.connect(dnsname, socket).await?;

    let hello = TunnelHello {
        config_name: client_config.read().as_ref().name.clone(),
        instance_id,
    };

    let encoded_hello: Vec<u8> = bincode::serialize(&hello).unwrap();

    stream
        .write_u16(encoded_hello.len().try_into().unwrap())
        .await?;
    stream.write_all(&encoded_hello).await?;

    client_listener(
        client_framed(stream),
        client_config,
        internal_server_connector,
        resolver.clone(),
    )
    .await?;

    Ok(())
}
