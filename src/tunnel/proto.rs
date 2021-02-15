use crate::{
    entities::{
        AccessKeyId, AccountName, ConfigName, InstanceId, ProfileName, ProjectName, SmolStr,
        TunnelId,
    },
    tunnel::connector::ConnectorRequest,
};
use bytes::BytesMut;
use futures::{
    channel::{mpsc, oneshot},
    pin_mut, select_biased,
    stream::StreamExt,
    task::{Context, Poll},
    Future, FutureExt, Sink, SinkExt, Stream,
};
use hashbrown::{hash_map::Entry, HashMap};
use parking_lot::Mutex;
use shadow_clone::shadow_clone;
use std::{
    convert::{TryFrom, TryInto},
    fmt,
    fmt::Formatter,
    io, mem,
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::Duration,
};
use stop_handle::{stop_handle, StopHandle};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf},
    macros::support::Pin,
    net::TcpStream,
    time::{sleep, timeout},
};
use tracing::{debug, info, warn};
use trust_dns_resolver::TokioAsyncResolver;

use crate::{
    config_core::ClientConfig,
    tunnel::{
        connector::{Compression, ConnectTarget, Connector},
        mixed_channel::to_async_rw,
        Error, MixedChannel,
    },
};
use lru_time_cache::LruCache;
use parking_lot::RwLock;
use rand::{thread_rng, Rng};
use rw_stream_sink::RwStreamSink;
use serde::{Deserialize, Serialize};
use tokio_util::compat::FuturesAsyncReadCompatExt;

#[derive(Serialize, Deserialize, Debug)]
pub enum RejectionReason {
    ConnectionRefused { error_message: String },
    UpstreamNotFound,
}

impl fmt::Display for RejectionReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RejectionReason::ConnectionRefused { error_message } => {
                write!(f, "connection refused: {}", error_message)
            }
            RejectionReason::UpstreamNotFound => write!(f, "upstream not found"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TunnelHello {
    pub config_name: ConfigName,
    pub account_name: AccountName,
    pub project_name: ProjectName,
    pub instance_id: InstanceId,
    pub access_key_id: AccessKeyId,
    pub secret_access_key: SmolStr,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum TunnelHelloResponse {
    Ok { tunnel_id: TunnelId },
    Err { msg: String },
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct Slot(u32);

impl fmt::Display for Slot {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<u64> for Slot {
    type Error = Error;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        if value > MAX_SLOT_NUM {
            return Err(Error::SlotOverflow);
        }

        Ok(Slot(value.try_into()?))
    }
}

impl TryFrom<u32> for Slot {
    type Error = Error;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        if u64::from(value) > MAX_SLOT_NUM {
            return Err(Error::SlotOverflow);
        }

        Ok(Slot(value))
    }
}

impl Slot {
    pub fn into_inner(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum CommonHeader {
    DataPlain,
    DataCompressed,
    Closed,
    Ping,
    Pong,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ServerHeader {
    ConnectRequest,
    TunnelClose,
    Common(CommonHeader),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ServerPacket {
    pub(crate) header: ServerHeader,
    pub(crate) slot: Slot,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ClientHeader {
    Accepted,
    Rejected,
    Common(CommonHeader),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ClientPacket {
    pub(crate) header: ClientHeader,
    pub(crate) slot: Slot,
}

pub const COMMON_CODE_DATA_PLAIN: u8 = 0;
pub const COMMON_CODE_DATA_COMPRESSED: u8 = 1;
pub const COMMON_CODE_CLOSED: u8 = 2;
pub const COMMON_CODE_PING: u8 = 3;
pub const COMMON_CODE_PONG: u8 = 4;

pub const CLIENT_CODE_ACCEPTED: u8 = MAX_CODE_VALUE as u8;
pub const CLIENT_CODE_REJECTED: u8 = (MAX_CODE_VALUE - 1) as u8;

pub const SERVER_CODE_CONNECT_REQUEST: u8 = MAX_CODE_VALUE as u8;
pub const SERVER_CODE_TUNNEL_CLOSE: u8 = (MAX_CODE_VALUE - 1) as u8;

pub const HEADER_BYTES: usize = 3;
pub const CODE_BITS_RESERVED: u64 = 4;
pub const MAX_CODE_VALUE: u64 = (1 << CODE_BITS_RESERVED) - 1;
pub const MAX_HEADER_CODE: u64 = 0xffffff;
pub const MAX_SLOT_NUM: u64 = MAX_HEADER_CODE >> 4;
//3 bytes - 4 bits, reserved for codes
pub const MAX_PAYLOAD_LEN: usize = u16::MAX as usize;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

enum Compressor {
    Plain,
    Zstd(zstd::block::Compressor),
}

impl Compressor {
    pub fn new(compression: Compression) -> Self {
        match compression {
            Compression::Plain => Compressor::Plain,
            Compression::Zstd => Compressor::Zstd(Default::default()),
        }
    }

    pub fn compress(&mut self, buf: Vec<u8>) -> (Vec<u8>, bool) {
        match self {
            Compressor::Plain => (buf, false),
            Compressor::Zstd(compressor) => {
                let compressed = compressor.compress(&buf, 0).expect("Unable to compress");

                // info!(
                //     "compress ratio = {}",
                //     buf.len() as f32 / compressed.len() as f32
                // );
                if compressed.len() < buf.len() {
                    (compressed, true)
                } else {
                    (buf, false)
                }
            }
        }
    }
}

enum Decompressor {
    Plain,
    Zstd(zstd::block::Decompressor),
}

impl Decompressor {
    pub fn new(compression: Compression) -> Self {
        match compression {
            Compression::Plain => Decompressor::Plain,
            Compression::Zstd => Decompressor::Zstd(Default::default()),
        }
    }

    pub fn decompress(&mut self, buf: Vec<u8>) -> Result<Vec<u8>, io::Error> {
        match self {
            Decompressor::Plain => Ok(buf),
            Decompressor::Zstd(compressor) => compressor.decompress(&buf, MAX_PAYLOAD_LEN),
        }
    }
}

struct Compressors {
    compressor: Compressor,
    decompressor: Decompressor,
}

impl Compressors {
    pub fn new(compression: Compression) -> Self {
        Compressors {
            compressor: Compressor::new(compression),
            decompressor: Decompressor::new(compression),
        }
    }
}

#[derive(Clone)]
pub struct Connection {
    stop_handle: StopHandle<()>,
    tunnel_to_tcp_tx: mpsc::Sender<Vec<u8>>,
    compressors: Arc<Mutex<Compressors>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectRequestPayload {
    target: ConnectTarget,
    compression: Compression,
}

// FIXME: abstraction is clearly broken here. we should not access client_config and
// handle particular request
pub async fn client_listener(
    tunnel: impl Stream<Item = Result<(ServerPacket, Vec<u8>), Error>>
        + Sink<(ClientPacket, Vec<u8>), Error = Error>
        + Send
        + 'static,
    client_config: Arc<RwLock<ClientConfig>>,
    mut internal_server_connector: mpsc::Sender<RwStreamSink<MixedChannel>>,
    active_profile: &Option<ProfileName>,
    resolver: TokioAsyncResolver,
) -> Result<bool, crate::tunnel::error::Error> {
    let storage = Arc::new(Mutex::new(HashMap::<Slot, Connection>::new()));
    let just_closed_by_us = Arc::new(Mutex::new(LruCache::<Slot, ()>::with_expiry_duration(
        Duration::from_secs(5),
    )));

    let (tx, mut rx) = tunnel.split();

    let (outgoing_messages_tx, outgoing_messages_rx) = mpsc::channel::<(_, Vec<u8>)>(16);
    let ping_period = Duration::from_secs(5);
    let wait_pong_timeout = ping_period * 3;

    let (mut received_pongs_tx, received_pongs_rx) = mpsc::channel(1);

    let periodic_pinger = {
        shadow_clone!(mut outgoing_messages_tx);

        #[allow(unreachable_code)]
        async move {
            loop {
                sleep(ping_period).await;
                let payload = vec![];
                outgoing_messages_tx
                    .send((
                        ClientPacket {
                            header: ClientHeader::Common(CommonHeader::Ping),
                            slot: 0u32.try_into().unwrap(),
                        },
                        payload,
                    ))
                    .await?;
            }

            Ok::<_, crate::tunnel::error::Error>(())
        }
    }
    .fuse();

    let pongs_timeout = async move {
        let pongs = tokio_stream::StreamExt::timeout(received_pongs_rx, wait_pong_timeout);

        pin_mut!(pongs);

        while let Some(Ok(())) = pongs.next().await {}
    };

    let read_future = {
        shadow_clone!(client_config, storage, outgoing_messages_tx, just_closed_by_us, mut outgoing_messages_tx, active_profile);

        async move {
            while let Some(res) = rx.next().await {
                match res {
                    Ok((ServerPacket { header, slot }, payload)) => {
                        match header {
                            ServerHeader::TunnelClose => {
                                return Ok(false);
                            }
                            ServerHeader::ConnectRequest => {
                                let req = bincode::deserialize::<ConnectRequestPayload>(&payload)?;
                                let target = req.target;
                                let compression = req.compression;
                                match target {
                                    ConnectTarget::Upstream(upstream) => {
                                        tokio::spawn({
                                            shadow_clone!(resolver, storage, client_config, mut outgoing_messages_tx, just_closed_by_us, active_profile);

                                            async move {
                                                let maybe_upstream_target = client_config.read().resolve_upstream(&upstream, &active_profile);

                                                if let Some(upstream_target) = maybe_upstream_target {
                                                    let host = upstream_target.get_host();
                                                    let ip_addr = if let Ok(ip_addr) = host.parse::<IpAddr>() {
                                                        ip_addr
                                                    } else {
                                                        let r = async {
                                                            resolver
                                                                .lookup_ip(host.as_str())
                                                                .await
                                                                .map_err(|e| {
                                                                    warn!("resolver error: {}", e);
                                                                    crate::tunnel::Error::UpstreamResolveError {
                                                                        upstream: upstream.clone(),
                                                                        host: host.as_str().to_string(),
                                                                    }
                                                                })
                                                                .into_iter()
                                                                .next()
                                                                .ok_or_else(|| crate::tunnel::Error::UpstreamResolveError {
                                                                    upstream: upstream.clone(),
                                                                    host: host.as_str().to_string(),
                                                                })?
                                                                .into_iter()
                                                                .next()
                                                                .ok_or_else(|| crate::tunnel::Error::UpstreamResolveError {
                                                                    upstream,
                                                                    host: host.as_str().to_string(),
                                                                })
                                                        }.await;

                                                        match r {
                                                            Ok(r) => r,
                                                            Err(e) => {
                                                                warn!("error resolving upstream: {}", e);

                                                                let payload = bincode::serialize(&RejectionReason::ConnectionRefused {
                                                                    error_message: e.to_string(),
                                                                }).unwrap();

                                                                outgoing_messages_tx.send((
                                                                    ClientPacket {
                                                                        header: ClientHeader::Rejected,
                                                                        slot,
                                                                    },
                                                                    payload,
                                                                )).await?;

                                                                return Err(e);
                                                            }
                                                        }
                                                    };
                                                    let connect_to: SocketAddr = (ip_addr, upstream_target.addr.port).into();
                                                    let res = timeout(
                                                        CONNECT_TIMEOUT,
                                                        TcpStream::connect(connect_to),
                                                    ).await;

                                                    match res {
                                                        Ok(Ok(mut tcp)) => {
                                                            outgoing_messages_tx.send((
                                                                ClientPacket {
                                                                    header: ClientHeader::Accepted,
                                                                    slot,
                                                                },
                                                                Default::default())
                                                            ).await?;

                                                            let (tunnel_to_tcp_tx, mut tunnel_to_tcp_rx) = mpsc::channel(4);
                                                            let (stop_handle, mut stop_wait) = stop_handle::<()>();

                                                            let compressors = Arc::new(Mutex::new(Compressors::new(compression)));

                                                            storage.lock().insert(
                                                                slot,
                                                                Connection {
                                                                    stop_handle,
                                                                    tunnel_to_tcp_tx,
                                                                    compressors: compressors.clone(),
                                                                });

                                                            tokio::spawn({
                                                                shadow_clone!(storage, outgoing_messages_tx, just_closed_by_us, compressors);

                                                                async move {
                                                                    let (mut from_tcp, mut to_tcp) = tcp.split();

                                                                    let forward_to_tunnel = {
                                                                        shadow_clone!(outgoing_messages_tx, compressors);

                                                                        async move {
                                                                            loop {
                                                                                shadow_clone!(mut outgoing_messages_tx);
                                                                                let mut buf = BytesMut::new();
                                                                                buf.resize(MAX_PAYLOAD_LEN, 0);

                                                                                let num_bytes = from_tcp.read(&mut buf).await?;

                                                                                if num_bytes == 0 {
                                                                                    break;
                                                                                }

                                                                                buf.truncate(num_bytes);

                                                                                let buf = buf.freeze().to_vec();
                                                                                let (maybe_compressed, is_compressed) = compressors
                                                                                    .lock()
                                                                                    .compressor
                                                                                    .compress(buf);

                                                                                outgoing_messages_tx.send((
                                                                                    ClientPacket {
                                                                                        header: ClientHeader::Common(if is_compressed { CommonHeader::DataCompressed } else { CommonHeader::DataPlain }),
                                                                                        slot,
                                                                                    },
                                                                                    maybe_compressed
                                                                                ))
                                                                                    .await
                                                                                    .map_err(|_|
                                                                                        io::Error::new(io::ErrorKind::Other, "channel closed")
                                                                                    )?;
                                                                            }

                                                                            Ok::<(), io::Error>(())
                                                                        }.fuse()
                                                                    };

                                                                    let forward_to_connection = {
                                                                        shadow_clone!(compressors);

                                                                        async move {
                                                                            while let Some(buf) = tunnel_to_tcp_rx.next().await {
                                                                                to_tcp.write_all(&buf).await?;
                                                                            }
                                                                            Ok::<(), io::Error>(())
                                                                        }.fuse()
                                                                    };

                                                                    let forwarders = {
                                                                        shadow_clone!(mut outgoing_messages_tx, just_closed_by_us);

                                                                        async move {
                                                                            let res = tokio::select! {
                                                                                res = forward_to_tunnel => res,
                                                                                res = forward_to_connection => res,
                                                                            };

                                                                            debug!("connection on slot {} closed {:?}", slot, res);

                                                                            if storage.lock().remove(&slot).is_some() {
                                                                                let _ = outgoing_messages_tx.send((
                                                                                    ClientPacket {
                                                                                        header: ClientHeader::Common(CommonHeader::Closed),
                                                                                        slot,
                                                                                    },
                                                                                    Default::default()
                                                                                )).await;
                                                                            };

                                                                            just_closed_by_us.lock().insert(slot, ());
                                                                        }.fuse()
                                                                    };

                                                                    pin_mut!(forwarders);

                                                                    select_biased! {
                                                                    _ = forwarders => {},
                                                                    _ = stop_wait => {},
                                                                }
                                                                    debug!("slot connection {} closed", slot);
                                                                }
                                                            });
                                                        }
                                                        Ok(Err(e)) => {
                                                            info!("error connecting to {:?}. error: {:?}", connect_to, e);

                                                            let payload = bincode::serialize(&RejectionReason::ConnectionRefused {
                                                                error_message: e.to_string(),
                                                            }).unwrap();

                                                            outgoing_messages_tx.send((
                                                                ClientPacket {
                                                                    header: ClientHeader::Rejected,
                                                                    slot,
                                                                },
                                                                payload,
                                                            )).await?;
                                                        }
                                                        Err(e) => {
                                                            info!("error connecting: {}", e);
                                                            let payload = bincode::serialize(&RejectionReason::ConnectionRefused {
                                                                error_message: "timeout".to_string(),
                                                            }).unwrap();

                                                            outgoing_messages_tx.send((
                                                                ClientPacket {
                                                                    header: ClientHeader::Rejected,
                                                                    slot,
                                                                },
                                                                payload,
                                                            )).await?;
                                                        }
                                                    }
                                                } else {
                                                    debug!("error connecting to {:?}. not found in config", upstream);
                                                    let payload = bincode::serialize(&RejectionReason::UpstreamNotFound)
                                                        .unwrap();

                                                    outgoing_messages_tx.send((
                                                        ClientPacket {
                                                            header: ClientHeader::Rejected,
                                                            slot,
                                                        },
                                                        payload,
                                                    )).await?;
                                                }
                                                Ok::<_, crate::tunnel::Error>(())
                                            }
                                        });
                                    }
                                    ConnectTarget::Internal(_) => {
                                        let (ch, mut tx, mut rx) = MixedChannel::new(16, 16);

                                        tokio::spawn({
                                            shadow_clone!(internal_server_connector, mut outgoing_messages_tx, storage, just_closed_by_us);

                                            async move {
                                                outgoing_messages_tx.send((
                                                    ClientPacket {
                                                        header: ClientHeader::Accepted,
                                                        slot,
                                                    },
                                                    Default::default())
                                                ).await?;

                                                let (tunnel_to_tcp_tx, mut tunnel_to_tcp_rx) = mpsc::channel(4);
                                                let (stop_handle, mut stop_wait) = stop_handle::<()>();

                                                let compressors = Arc::new(Mutex::new(Compressors::new(compression)));

                                                storage.lock().insert(
                                                    slot,
                                                    Connection {
                                                        stop_handle,
                                                        tunnel_to_tcp_tx,
                                                        compressors: compressors.clone(),
                                                    });

                                                tokio::spawn({
                                                    shadow_clone!(compressors, storage, storage, outgoing_messages_tx);

                                                    async move {
                                                        let forward_to_tunnel = {
                                                            shadow_clone!(mut outgoing_messages_tx, compressors);

                                                            async move {
                                                                while let Some(buf) = rx.next().await {
                                                                    for chunk in buf.chunks(MAX_PAYLOAD_LEN) {
                                                                        let buf = chunk.to_vec();
                                                                        let (maybe_compressed, is_compressed) = compressors
                                                                            .lock()
                                                                            .compressor
                                                                            .compress(buf);

                                                                        outgoing_messages_tx.send((
                                                                            ClientPacket {
                                                                                header: ClientHeader::Common(if is_compressed { CommonHeader::DataCompressed } else { CommonHeader::DataPlain }),
                                                                                slot,
                                                                            }, maybe_compressed
                                                                        ))
                                                                            .await
                                                                            .map_err(|_|
                                                                                io::Error::new(io::ErrorKind::Other, "channel closed")
                                                                            )?;
                                                                    }
                                                                }

                                                                Ok::<(), io::Error>(())
                                                            }.fuse()
                                                        };

                                                        let forward_to_internal_server = {
                                                            shadow_clone!(compressors);

                                                            async move {
                                                                while let Some(buf) = tunnel_to_tcp_rx.next().await {
                                                                    tx
                                                                        .send(buf)
                                                                        .await
                                                                        .map_err(|_| io::Error::new(io::ErrorKind::Other, "channel closed"))?
                                                                }
                                                                Ok::<(), io::Error>(())
                                                            }.fuse()
                                                        };

                                                        let forwarders = {
                                                            shadow_clone!(mut outgoing_messages_tx);

                                                            async move {
                                                                let res = tokio::select! {
                                                                    res = forward_to_tunnel => res,
                                                                    res = forward_to_internal_server => res,
                                                                };

                                                                debug!("connection on slot {} closed {:?}", slot, res);

                                                                if storage.lock().remove(&slot).is_some() {
                                                                    let _ = outgoing_messages_tx.send((
                                                                        ClientPacket {
                                                                            header: ClientHeader::Common(CommonHeader::Closed),
                                                                            slot,
                                                                        },
                                                                        Default::default()
                                                                    )).await;
                                                                };

                                                                just_closed_by_us.lock().insert(slot, ());
                                                            }.fuse()
                                                        };

                                                        pin_mut!(forwarders);

                                                        select_biased! {
                                                        _ = forwarders => {},
                                                        _ = stop_wait => {},
                                                    }
                                                        debug!("slot connection {} closed", slot);
                                                    }
                                                });

                                                Ok::<_, crate::tunnel::Error>(())
                                            }
                                        });

                                        internal_server_connector.send(RwStreamSink::new(ch)).await?;
                                    }
                                }
                            }
                            ServerHeader::Common(CommonHeader::DataPlain) | ServerHeader::Common(CommonHeader::DataCompressed) => {
                                let maybe_slot_info = storage
                                    .lock()
                                    .get(&slot)
                                    .map(|client_connection| {
                                        (client_connection
                                             .tunnel_to_tcp_tx
                                             .clone(),
                                         client_connection
                                             .compressors
                                             .clone()
                                        )
                                    });
                                if let Some((mut tunnel_to_tcp_tx, compressors)) = maybe_slot_info {
                                    let decompressed = if let ServerHeader::Common(CommonHeader::DataCompressed) = header {
                                        compressors
                                            .lock()
                                            .decompressor
                                            .decompress(payload)?
                                    } else {
                                        payload
                                    };

                                    tunnel_to_tcp_tx
                                        .send(decompressed)
                                        .await?;
                                } else if just_closed_by_us.lock().get(&slot).is_none() {
                                    warn!("unknown slot {}, closing connection", slot);
                                    return Err(Error::UnknownSlot(slot));
                                } else {
                                    debug!("ignore unknown slot, possible race-condition");
                                }
                            }
                            ServerHeader::Common(CommonHeader::Closed) => {
                                if let Some(slot) = storage
                                    .lock()
                                    .remove(&slot)
                                {
                                    slot
                                        .stop_handle
                                        .stop(());
                                } else if just_closed_by_us.lock().get(&slot).is_none() {
                                    warn!("unknown slot {}, closing connection", slot);
                                    return Err(Error::UnknownSlot(slot));
                                } else {
                                    debug!("ignore unknown slot, possible race-condition");
                                }
                            }
                            ServerHeader::Common(CommonHeader::Ping) => {
                                let payload = vec![];
                                outgoing_messages_tx.send((
                                    ClientPacket {
                                        header: ClientHeader::Common(CommonHeader::Pong),
                                        slot,
                                    },
                                    payload,
                                )).await?;
                            },
                            ServerHeader::Common(CommonHeader::Pong) => {
                                received_pongs_tx.send(()).await?;
                            },
                        }
                    }
                    Err(e) => {
                        warn!("error reading from sink {}", e);
                        break;
                    }
                }
            }

            Ok::<bool, crate::tunnel::Error>(true)
        }
    }.fuse();

    let write_future = outgoing_messages_rx
        .map(|a| Ok((a.0, a.1)))
        .forward(tx)
        .fuse();

    let res = tokio::select! {
        r = read_future => r,
        _r = write_future => {
            Ok::<bool, crate::tunnel::error::Error>(true)
        },
        _ = pongs_timeout => {
            warn!("timeout waiting for pong on tunnel. closing");
            Ok(true)
        },
        _r = periodic_pinger => Ok::<bool, crate::tunnel::error::Error>(true),
    };

    info!("tunnel closed with result {:?}", res);

    Ok(res?)
}

pub enum ServerConnection {
    Initiating((oneshot::Sender<Box<dyn Conn>>, Compression)),
    Established(Connection),
}

impl ServerConnection {
    fn established(&self) -> Option<&Connection> {
        match self {
            ServerConnection::Initiating(_) => None,
            ServerConnection::Established(conn) => Some(conn),
        }
    }

    fn into_established(self) -> Option<Connection> {
        match self {
            ServerConnection::Initiating(_) => None,
            ServerConnection::Established(conn) => Some(conn),
        }
    }

    fn is_initiating(&self) -> bool {
        match self {
            ServerConnection::Initiating(_) => true,
            ServerConnection::Established(_) => false,
        }
    }

    fn get_compression(&self) -> Option<Compression> {
        match self {
            ServerConnection::Initiating((_, compression)) => Some(*compression),
            ServerConnection::Established(_) => None,
        }
    }

    fn take_initiating_stream(self) -> Option<oneshot::Sender<Box<dyn Conn>>> {
        match self {
            ServerConnection::Initiating((tcp_stream, _)) => Some(tcp_stream),
            ServerConnection::Established(_) => None,
        }
    }
}

pub trait Conn: AsyncRead + AsyncWrite + Send + Unpin + 'static {}

impl<T> Conn for T where T: AsyncRead + AsyncWrite + Send + Unpin + 'static {}

pub fn server_connection(
    transport: impl Stream<Item = Result<(ClientPacket, Vec<u8>), Error>>
        + Sink<(ServerPacket, Vec<u8>), Error = Error>
        + Send
        + 'static,
) -> (
    impl Future<Output = Result<(), crate::tunnel::Error>> + Send + 'static,
    crate::tunnel::connector::Connector,
) {
    let storage = Arc::new(Mutex::new(HashMap::<Slot, ServerConnection>::new()));
    let (new_connection_req_tx, mut new_connection_req_rx) = mpsc::channel(2);
    let just_closed_by_us = Arc::new(Mutex::new(LruCache::<Slot, ()>::with_expiry_duration(
        Duration::from_secs(5),
    )));

    let (mut received_pongs_tx, received_pongs_rx) = mpsc::channel(1);

    let ping_period = Duration::from_secs(5);
    let wait_pong_timeout = ping_period * 3;

    let f = {
        async move {
            let slot_counter = Mutex::new(0u32);

            let (tx, mut rx) = transport.split();

            let (outgoing_messages_tx, outgoing_messages_rx) = mpsc::channel(16);

            let accept_connect_future = {
                shadow_clone!(mut outgoing_messages_tx, storage);

                #[allow(unreachable_code)]
                async move {
                    while let Some(ConnectorRequest {
                        tx: ready_async_channel_tx,
                        target: connect_target,
                        compression,
                    }) = new_connection_req_rx.next().await
                    {
                        let slot = {
                            let mut locked_slot_counter = slot_counter.lock();

                            *locked_slot_counter += 1;
                            loop {
                                if *locked_slot_counter > MAX_SLOT_NUM as u32 {
                                    *locked_slot_counter = 0;
                                }
                                let slot = (*locked_slot_counter).try_into().unwrap();
                                if !storage.lock().contains_key(&slot) {
                                    break slot;
                                } else {
                                    *locked_slot_counter =
                                        thread_rng().gen_range(0..MAX_SLOT_NUM as u32);
                                }
                            }
                        };
                        storage.lock().insert(
                            slot,
                            ServerConnection::Initiating((ready_async_channel_tx, compression)),
                        );

                        outgoing_messages_tx
                            .send((
                                ServerPacket {
                                    header: ServerHeader::ConnectRequest,
                                    slot,
                                },
                                bincode::serialize(&ConnectRequestPayload {
                                    target: connect_target,
                                    compression,
                                })
                                .unwrap(),
                            ))
                            .await
                            .map_err(|_| io::Error::new(io::ErrorKind::Other, "channel closed"))?;
                    }

                    Ok::<(), crate::tunnel::Error>(())
                }
                .fuse()
            };

            let read_future = {
                shadow_clone!(storage, mut outgoing_messages_tx);

                async move {
                    while let Some(res) = rx.next().await {
                        match res {
                            Ok((ClientPacket { header, slot }, payload)) => {
                                match header {
                                    ClientHeader::Accepted => {
                                        let s = &mut *storage.lock();

                                        match s.entry(slot) {
                                            Entry::Occupied(mut e) => {
                                                if e.get().is_initiating() {
                                                    let (tunnel_to_tcp_tx, mut tunnel_to_channel) = mpsc::channel::<Vec<u8>>(4);
                                                    let (stop_handle, mut stop_wait) = stop_handle::<()>();

                                                    let compression = e.get().get_compression().unwrap();

                                                    let compressors = Arc::new(Mutex::new(Compressors::new(compression)));

                                                    let ready_connection_resolver = mem::replace(e.get_mut(), ServerConnection::Established(Connection {
                                                        stop_handle,
                                                        tunnel_to_tcp_tx,
                                                        compressors: compressors.clone(),
                                                    })).take_initiating_stream().unwrap();

                                                    let (channel, mut from_tunnel_tx, mut to_tunnel_rx) = to_async_rw(16, 16);

                                                    ready_connection_resolver
                                                        .send(Box::new(channel.compat()))
                                                        .map_err(|_| io::Error::new(io::ErrorKind::Other, "tunnel closed: could ot sed to read_connection_resolver"))?;

                                                    tokio::spawn({
                                                        shadow_clone!(storage, compressors, outgoing_messages_tx, just_closed_by_us, just_closed_by_us);

                                                        async move {
                                                            let forward_to_tunnel = {
                                                                shadow_clone!(outgoing_messages_tx, compressors);

                                                                async move {
                                                                    while let Some(buf) = to_tunnel_rx.next().await {
                                                                        for chunk in buf.chunks(MAX_PAYLOAD_LEN) {
                                                                            shadow_clone!(mut outgoing_messages_tx);

                                                                            let (maybe_compressed, is_compressed) = compressors
                                                                                .lock()
                                                                                .compressor
                                                                                .compress(chunk.to_vec());

                                                                            outgoing_messages_tx.send((
                                                                                ServerPacket {
                                                                                    header: ServerHeader::Common(if is_compressed { CommonHeader::DataCompressed } else { CommonHeader::DataPlain }),
                                                                                    slot,
                                                                                }, maybe_compressed
                                                                            )).await
                                                                                .map_err(|_|
                                                                                    io::Error::new(io::ErrorKind::Other, "channel closed")
                                                                                )?;
                                                                        }
                                                                    }

                                                                    Ok::<(), io::Error>(())
                                                                }.fuse()
                                                            };

                                                            let forward_to_connection = {
                                                                shadow_clone!(compressors);

                                                                async move {
                                                                    while let Some(buf) = tunnel_to_channel.next().await {
                                                                        from_tunnel_tx
                                                                            .send(buf)
                                                                            .await
                                                                            .map_err(|_| io::Error::new(io::ErrorKind::Other, "tunnel closed: could not send to from_tunnel_tx"))
                                                                            ?;
                                                                    }
                                                                    Ok::<(), io::Error>(())
                                                                }.fuse()
                                                            };

                                                            let forwarders = {
                                                                shadow_clone!(mut outgoing_messages_tx, just_closed_by_us);

                                                                async move {
                                                                    let res = tokio::select! {
                                                                        res = forward_to_tunnel => res,
                                                                        res = forward_to_connection => res,
                                                                    };

                                                                    debug!("connection on slot {} closed {:?}", slot, res);

                                                                    if storage.lock().remove(&slot).is_some() {
                                                                        outgoing_messages_tx.send((
                                                                            ServerPacket {
                                                                                header: ServerHeader::Common(CommonHeader::Closed),
                                                                                slot,
                                                                            },
                                                                            Default::default()
                                                                        )).await?;
                                                                    }

                                                                    just_closed_by_us.lock().insert(slot, ());

                                                                    res?;

                                                                    Ok::<(), crate::tunnel::Error>(())
                                                                }.fuse()
                                                            };

                                                            pin_mut!(forwarders);

                                                            let r = select_biased! {
                                                                r = forwarders => r,
                                                                _ = stop_wait => Ok(()),
                                                            };
                                                            debug!("slot connection {} closed", slot);

                                                            r
                                                        }
                                                    });
                                                } else {
                                                    warn!("received Accepted while connection is not in Initiating state");
                                                    return Err(Error::ConnectionHandshakeOnEstablishedConnection);
                                                }
                                            }
                                            Entry::Vacant(_) => {
                                                if just_closed_by_us.lock().get(&slot).is_none() {
                                                    warn!("unknown slot {}, closing connection", slot);
                                                    return Err(Error::UnknownSlot(slot));
                                                } else {
                                                    debug!("ignore unknown slot, possible race-condition");
                                                }
                                            }
                                        }
                                    }
                                    ClientHeader::Rejected => {
                                        let error = bincode::deserialize::<RejectionReason>(&payload)?;

                                        info!("connection rejected with reason: {}", error);

                                        match storage
                                            .lock()
                                            .entry(slot)
                                        {
                                            Entry::Occupied(a) => {
                                                if a.get().is_initiating() {
                                                    a.remove_entry();
                                                } else {
                                                    warn!("received Rejected while connection is not in Initiating state");
                                                    return Err(Error::ConnectionHandshakeOnEstablishedConnection);
                                                }
                                            }
                                            Entry::Vacant(_) => {
                                                if just_closed_by_us.lock().get(&slot).is_none() {
                                                    warn!("unknown slot {}, closing connection", slot);
                                                    return Err(Error::UnknownSlot(slot));
                                                } else {
                                                    debug!("ignore unknown slot, possible race-condition");
                                                }
                                            }
                                        }
                                    }
                                    ClientHeader::Common(CommonHeader::DataPlain) | ClientHeader::Common(CommonHeader::DataCompressed) => {
                                        let res = storage
                                            .lock()
                                            .get(&slot)
                                            .map(|r| r.established().cloned());
                                        if let Some(slot) = res {
                                            if let Some(conn) = slot {
                                                let decompressed = if let ClientHeader::Common(CommonHeader::DataCompressed) = header {
                                                    conn
                                                        .compressors
                                                        .lock()
                                                        .decompressor
                                                        .decompress(payload)?
                                                } else {
                                                    payload
                                                };

                                                conn
                                                    .tunnel_to_tcp_tx
                                                    .clone()
                                                    .send(decompressed)
                                                    .await?;
                                            } else {
                                                warn!("received data while connection is not in established state");
                                                return Err(Error::CommandOnInitiatingConnection);
                                            }
                                        } else if just_closed_by_us.lock().get(&slot).is_none() {
                                            warn!("unknown slot {}, closing connection", slot);
                                            return Err(Error::UnknownSlot(slot));
                                        } else {
                                            debug!("ignore unknown slot, possible race-condition");
                                        };
                                    }
                                    ClientHeader::Common(CommonHeader::Closed) => {
                                        if let Some(slot) = storage
                                            .lock()
                                            .remove(&slot)
                                        {
                                            if let Some(conn) = slot.into_established() {
                                                conn.stop_handle.stop(());
                                            } else {
                                                warn!("closed command received during connection initialization");
                                                return Err(Error::CommandOnInitiatingConnection);
                                            }
                                        }
                                    }
                                    ClientHeader::Common(CommonHeader::Ping) => {
                                        let payload = vec![];
                                        outgoing_messages_tx.send((
                                            ServerPacket {
                                                header: ServerHeader::Common(CommonHeader::Pong),
                                                slot,
                                            },
                                            payload,
                                        )).await?;
                                    }
                                    ClientHeader::Common(CommonHeader::Pong) => {
                                        received_pongs_tx.send(()).await?;
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("error reading from sink {}", e);
                                break;
                            }
                        }
                    }

                    Ok::<(), crate::tunnel::Error>(())
                }
            }.fuse();

            let write_future = outgoing_messages_rx.map(Ok).forward(tx).fuse();
            let pongs_timeout = async move {
                let pongs = tokio_stream::StreamExt::timeout(received_pongs_rx, wait_pong_timeout);

                pin_mut!(pongs);

                while let Some(Ok(())) = pongs.next().await {}
            }
            .fuse();

            #[allow(unreachable_code)]
            let periodic_pinger = {
                shadow_clone!(mut outgoing_messages_tx);

                async move {
                    loop {
                        sleep(ping_period).await;
                        let payload = vec![];
                        outgoing_messages_tx
                            .send((
                                ServerPacket {
                                    header: ServerHeader::Common(CommonHeader::Ping),
                                    slot: 0u32.try_into().unwrap(),
                                },
                                payload,
                            ))
                            .await?;
                    }

                    Ok(())
                }
            }
            .fuse();

            pin_mut!(read_future);
            pin_mut!(write_future);
            pin_mut!(accept_connect_future);
            pin_mut!(pongs_timeout);
            pin_mut!(periodic_pinger);

            select_biased! {
                r = accept_connect_future => r,
                r = read_future => r,
                r = write_future => r,
                r = periodic_pinger => r,
                () = pongs_timeout => {
                    warn!("timeout waiting for pong on tunnel. closing");
                    Ok(())
                }
            }
        }
    };

    (f, Connector::new(new_connection_req_tx))
}

pub struct TunneledConnection {
    inner: Box<dyn Conn>,
}

impl TunneledConnection {
    pub fn new(conn: Box<dyn Conn>) -> Self {
        TunneledConnection { inner: conn }
    }
}

impl hyper::client::connect::Connection for TunneledConnection {
    fn connected(&self) -> hyper::client::connect::Connected {
        hyper::client::connect::Connected::new()
    }
}

impl AsyncRead for TunneledConnection {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for TunneledConnection {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

#[cfg(test)]
mod test {
    use crate::config_core::CURRENT_VERSION;
    use std::net::{IpAddr, SocketAddr};

    use tokio::net::TcpListener;

    use crate::tunnel::framed::{client_framed, server_framed};

    use super::*;
    use crate::config_core::{
        refinable::Refinable, ClientConfig, ClientConfigRevision, UpstreamDefinition,
    };
    use std::collections::BTreeMap;
    use trust_dns_resolver::TokioHandle;

    #[tokio::test]
    async fn test_simple() {
        let buf1 = vec![1, 2, 3, 4, 5, 6];
        let buf2 = vec![7, 8, 9];
        let buf4 = vec![10; MAX_PAYLOAD_LEN * 2];
        let buf1_3 = vec![65, 66, 67];

        let resolver = TokioAsyncResolver::from_system_conf(TokioHandle).unwrap();

        let server_side_listener =
            TcpListener::bind(&SocketAddr::new(IpAddr::from([127, 0, 0, 1]), 0))
                .await
                .unwrap();
        let server_side_socket = server_side_listener.local_addr().unwrap();

        let send_handle = tokio::spawn({
            shadow_clone!(buf1, buf2, buf4, buf1_3);

            async move {
                let (server_side, _remote_addr) = server_side_listener.accept().await.unwrap();

                let (bg, connector) = server_connection(server_framed(server_side));

                tokio::spawn(bg);

                {
                    let mut tunneled1 = connector
                        .retrieve_connection(
                            "backend.upstream.exg".parse().unwrap(),
                            Compression::Zstd,
                        )
                        .await
                        .unwrap();
                    tunneled1.write_all(&buf1).await.unwrap();
                }

                let mut tunneled2 = connector
                    .retrieve_connection(
                        "backend.upstream.exg".parse().unwrap(),
                        Compression::Plain,
                    )
                    .await
                    .unwrap();
                tunneled2.write_all(&buf2).await.unwrap();

                {
                    let _tunneled3 = connector
                        .retrieve_connection(
                            "backend.upstream.exg".parse().unwrap(),
                            Compression::Zstd,
                        )
                        .await
                        .unwrap();
                }

                {
                    let mut tunneled4 = connector
                        .retrieve_connection(
                            "backend.upstream.exg".parse().unwrap(),
                            Compression::Plain,
                        )
                        .await
                        .unwrap();
                    tunneled4.write_all(&buf4).await.unwrap();
                }

                let mut buf = vec![];
                tunneled2.read_to_end(&mut buf).await.unwrap();
                assert_eq!(buf, buf1_3);
            }
        });

        let tcp_tunneled = TcpListener::bind(&SocketAddr::new(IpAddr::from([127, 0, 0, 1]), 0))
            .await
            .unwrap();
        let connect_to_addr = tcp_tunneled.local_addr().unwrap();

        let mut upstreams = BTreeMap::new();
        upstreams.insert(
            "backend".parse().unwrap(),
            UpstreamDefinition::on_default_host(connect_to_addr.port()),
        );

        let client_config = Arc::new(RwLock::new(
            ClientConfig {
                version: CURRENT_VERSION.clone(),
                revision: ClientConfigRevision(1),
                name: "my-config".parse().unwrap(),
                mount_points: Default::default(),
                upstreams,
                refinable: Refinable {
                    static_responses: Default::default(),
                    rescue: vec![],
                },
            }
            .into(),
        ));

        let (internal_server_connector, _new_conn_rx) = mpsc::channel(1);

        tokio::spawn({
            async move {
                let client_side = TcpStream::connect(&server_side_socket).await.unwrap();

                client_listener(
                    client_framed(client_side),
                    client_config,
                    internal_server_connector,
                    &None,
                    resolver,
                )
                .await
                .unwrap();
            }
        });

        let (mut accepted_connection1, _) = tcp_tunneled.accept().await.unwrap();
        let mut read_buf1 = vec![];
        accepted_connection1.read_buf(&mut read_buf1).await.unwrap();

        let (mut accepted_connection2, _) = tcp_tunneled.accept().await.unwrap();
        let mut read_buf2 = vec![];
        accepted_connection2.read_buf(&mut read_buf2).await.unwrap();

        let (_accepted_connection3, _) = tcp_tunneled.accept().await.unwrap();

        let (mut accepted_connection4, _) = tcp_tunneled.accept().await.unwrap();
        let mut read_buf4 = vec![0; MAX_PAYLOAD_LEN * 2];
        accepted_connection4
            .read_exact(&mut read_buf4)
            .await
            .unwrap();

        accepted_connection2.write_all(&buf1_3).await.unwrap();

        mem::drop(accepted_connection2);

        assert_eq!(buf1, read_buf1);
        assert_eq!(buf2, read_buf2);
        assert_eq!(buf4, read_buf4);

        send_handle.await.unwrap();
    }
}
