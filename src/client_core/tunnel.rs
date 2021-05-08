use std::io;

use crate::{
    access_tokens::{generate_jwt_token, JwtError},
    common_utils::tls::load_native_certs_safe,
    config_core::ClientConfig,
    entities::{AccessKeyId, AccountName, InstanceId, ProfileName, ProjectName, SmolStr},
    tunnel::{
        client_framed, client_listener, MixedChannel, TunnelHello, TunnelHelloResponse,
        ALPN_PROTOCOL,
    },
};
use core::time::Duration;
use futures::channel::mpsc;
use hashbrown::HashMap;
use parking_lot::RwLock;
use rand::{seq::IteratorRandom, thread_rng};
use rustls::ClientConfig as RustlsClientConfig;
use rw_stream_sink::RwStreamSink;
use std::{convert::TryInto, net::SocketAddr, sync::Arc};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tokio_rustls::{rustls, webpki::DNSNameRef, TlsConnector};
use tracing::{error, field, info, info_span};
use trust_dns_resolver::{error::ResolveError, TokioAsyncResolver};
use url::Url;
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
    ResolveError(#[from] Box<ResolveError>),

    #[error("no addresses resolved")]
    NothingResolved,

    #[error("invalid DNS name: `{_0}`")]
    BadDnsName(#[from] InvalidDNSNameError),

    #[error("bad http status code: `{_0}`")]
    BadHttpStatus(http::StatusCode),

    #[error("jwt error: `{_0}`")]
    Jwt(#[from] JwtError),
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
    additional_connection_params: &HashMap<SmolStr, SmolStr>,
    internal_server_connector: mpsc::Sender<RwStreamSink<MixedChannel>>,
    resolver: TokioAsyncResolver,
) -> Result<bool, Error> {
    let span = info_span!("spawn", tunnel_id = field::Empty);
    let (tunnel_id, stream) = tokio::time::timeout(Duration::from_secs(5), async {
        let gw_addrs = resolver
            .lookup_ip(gw_hostname.to_string())
            .await
            .map_err(Box::new)?;
        let gw_addr = gw_addrs
            .iter()
            .choose(&mut thread_rng())
            .ok_or(Error::NothingResolved)?;

        let socket = TcpStream::connect(SocketAddr::new(gw_addr, gw_port)).await?;
        let _ = socket.set_nodelay(true);
        let mut config = RustlsClientConfig::new();
        config.alpn_protocols = vec![ALPN_PROTOCOL.to_vec(), b"http/1.1".to_vec()];
        load_native_certs_safe(&mut config);
        let config = TlsConnector::from(Arc::new(config));
        let dns_name = DNSNameRef::try_from_ascii_str(&gw_hostname)?;

        let tls_stream = config.connect(dns_name, socket).await?;

        let (mut send_request, http_connection) = conn::Builder::new()
            .http2_only(false)
            .handshake(tls_stream)
            .await?;

        tokio::spawn(http_connection);

        let mut url: Url = format!("https://{}/exotun", gw_hostname).parse().unwrap();

        url.query_pairs_mut()
            .append_pair("exogress_version", crate::client_core::VERSION);

        for (k, v) in additional_connection_params.iter() {
            url.query_pairs_mut().append_pair(k.as_str(), v.as_str());
        }

        let req = http::Request::builder()
            .uri(url.as_str())
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

            jwt_token: generate_jwt_token(&secret_access_key, &access_key_id)?.into(),
        };

        let encoded_hello: Vec<u8> = serde_cbor::to_vec(&hello).unwrap();
        stream
            .write_u16(encoded_hello.len().try_into().unwrap())
            .await?;
        stream.write_all(&encoded_hello).await?;

        let resp_len = stream.read_u16().await?.into();
        let mut tunnel_hello_response = vec![0u8; resp_len];
        stream.read_exact(&mut tunnel_hello_response).await?;
        let hello_response = serde_cbor::from_slice::<TunnelHelloResponse>(&tunnel_hello_response)
            .map_err(crate::tunnel::Error::DecodeError)?;

        match hello_response {
            TunnelHelloResponse::Ok { tunnel_id } => Ok((tunnel_id, stream)),
            TunnelHelloResponse::Err { msg } => Err(Error::Rejected(msg)),
        }
    })
    .await
    .map_err(|_| Error::EstablishTimeout)??;

    span.record("tunnel_id", &tunnel_id.to_string().as_str());

    info!(parent: &span, "connected");

    let r = client_listener(
        client_framed(stream),
        client_config,
        internal_server_connector,
        active_profile,
        resolver.clone(),
    )
    .await?;

    info!(parent: &span, "closed successfully");

    Ok(r)
}

#[cfg(test)]
mod test {
    use serde::{Deserialize, Serialize};

    #[test]
    fn cbor_evolution() {
        #[derive(Debug, Serialize, Deserialize)]
        pub struct V1 {
            a: u16,
            b: String,
        }

        #[derive(Debug, Serialize, Deserialize)]
        pub struct V2 {
            a: u16,
            b: String,
            c: Option<String>,
        }

        #[derive(Debug, Serialize, Deserialize)]
        pub struct V3 {
            a: u16,
            #[serde(default)]
            c: String,
        }

        let v1 = V1 {
            a: 1,
            b: "v1".to_string(),
        };

        let v2 = V2 {
            a: 2,
            b: "v2".to_string(),
            c: Some("v2-introduced".to_string()),
        };

        let v1_serialized = serde_cbor::to_vec(&v1).unwrap();
        let v2_parsed_v1: V2 = serde_cbor::from_slice(&v1_serialized).unwrap();
        let v3_parsed_v1: V3 = serde_cbor::from_slice(&v1_serialized).unwrap();

        assert_eq!(v2_parsed_v1.a, 1);
        assert_eq!(v2_parsed_v1.b, "v1");

        assert_eq!(v3_parsed_v1.a, 1);
        assert_eq!(v3_parsed_v1.c, "");

        let v2_serialized = serde_cbor::to_vec(&v2).unwrap();
        let v3_parsed_v2: V3 = serde_cbor::from_slice(&v2_serialized).unwrap();

        assert_eq!(v3_parsed_v2.a, 2);
        assert_eq!(v3_parsed_v2.c, "v2-introduced");
    }
}
