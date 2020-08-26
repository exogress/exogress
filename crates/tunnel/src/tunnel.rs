use std::convert::{TryFrom, TryInto};
use std::fmt;
use std::fmt::Formatter;
use std::net::IpAddr;
use std::net::SocketAddr;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use std::{io, mem};

use bytes::{Bytes, BytesMut};
use futures::channel::{mpsc, oneshot};
use futures::select_biased;
use futures::stream::StreamExt;
use futures::task::{Context, Poll};
use futures::{pin_mut, Future, FutureExt, Sink, SinkExt, Stream};
use hashbrown::hash_map::Entry;
use hashbrown::HashMap;
use parking_lot::Mutex;
use stop_handle::{stop_handle, StopHandle};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::macros::support::Pin;
use tokio::net::TcpStream;
use tokio::time::timeout;
use trust_dns_resolver::TokioAsyncResolver;

use exogress_entities::{ConfigName, InstanceId};

use crate::connector::{ConnectTarget, Connector};
use crate::mixed_channel::to_async_rw;
use crate::{Error, MixedChannel};
use async_io_stream::IoStream;
use exogress_config_core::ClientConfig;
use parking_lot::RwLock;

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
    pub instance_id: InstanceId,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
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
    Data,
    Closed,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ServerHeader {
    ConnectRequest,
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

pub const COMMON_CODE_DATA: u8 = 0;
pub const COMMON_CODE_CLOSED: u8 = 1;

pub const CLIENT_CODE_ACCEPTED: u8 = 2;
pub const CLIENT_CODE_REJECTED: u8 = 3;

pub const SERVER_CODE_CONNECT_REQUEST: u8 = 2;

pub const HEADER_BYTES: usize = 3;
pub const CODE_BITS_RESERVED: u64 = 4;
pub const CODE_MASK: u64 = (1 << CODE_BITS_RESERVED) - 1;
pub const MAX_HEADER_CODE: u64 = 0xffffff;
pub const MAX_SLOT_NUM: u64 = MAX_HEADER_CODE >> 4; //3 bytes - 4 bits, reserved for codes

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone)]
pub struct Connection {
    stop_handle: StopHandle<()>,
    tunnel_to_tcp_tx: mpsc::Sender<Bytes>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectRequestPayload {
    target: ConnectTarget,
}

pub async fn client_listener(
    tunnel: impl Stream<Item = Result<(ServerPacket, BytesMut), Error>>
        + Sink<(ClientPacket, Bytes), Error = Error>
        + Send
        + 'static,
    client_config: Arc<RwLock<ClientConfig>>,
    mut internal_server_connector: mpsc::Sender<IoStream<MixedChannel, Bytes>>,
    resolver: TokioAsyncResolver,
) -> Result<(), crate::error::Error> {
    let storage = Arc::new(Mutex::new(HashMap::<Slot, Connection>::new()));

    let (tx, mut rx) = tunnel.split();

    let (outgoing_messages_tx, outgoing_messages_rx) = mpsc::channel(16);

    let read_future = {
        shadow_clone!(client_config);
        shadow_clone!(storage);
        shadow_clone!(outgoing_messages_tx);

        async move {
            while let Some(res) = rx.next().await {
                match res {
                    Ok((ServerPacket { header, slot }, payload)) => {
                        match header {
                            ServerHeader::ConnectRequest => {
                                match bincode::deserialize::<ConnectRequestPayload>(&payload)?.target {
                                    ConnectTarget::Upstream(upstream) => {
                                        tokio::spawn({
                                            shadow_clone!(resolver);
                                            shadow_clone!(storage);
                                            shadow_clone!(client_config);
                                            shadow_clone!(mut outgoing_messages_tx);

                                            async move {
                                                let maybe_upstream_target = client_config.read().resolve_upstream(&upstream);

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
                                                                    crate::Error::UpstreamResolveError {
                                                                        upstream: upstream.clone(),
                                                                        host: host.as_str().to_string(),
                                                                    }
                                                                })
                                                                .into_iter()
                                                                .next()
                                                                .ok_or_else(|| crate::Error::UpstreamResolveError {
                                                                    upstream: upstream.clone(),
                                                                    host: host.as_str().to_string(),
                                                                })?
                                                                .into_iter()
                                                                .next()
                                                                .ok_or_else(|| crate::Error::UpstreamResolveError {
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
                                                                    payload.into(),
                                                                )).await?;

                                                                return Err(e);
                                                            }
                                                        }
                                                    };
                                                    let connect_to: SocketAddr = (ip_addr, upstream_target.port).into();
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

                                                            storage.lock().insert(
                                                                slot,
                                                                Connection {
                                                                    stop_handle,
                                                                    tunnel_to_tcp_tx,
                                                                });

                                                            tokio::spawn({
                                                                shadow_clone!(storage);
                                                                shadow_clone!(outgoing_messages_tx);

                                                                async move {
                                                                    let (mut from_tcp, mut to_tcp) = tcp.split();

                                                                    let forward_to_tunnel = {
                                                                        shadow_clone!(outgoing_messages_tx);

                                                                        async move {
                                                                            loop {
                                                                                shadow_clone!(mut outgoing_messages_tx);
                                                                                let mut buf = BytesMut::new();
                                                                                buf.resize(0xffff, 0);

                                                                                let num_bytes = from_tcp.read(&mut buf).await?;

                                                                                if num_bytes == 0 {
                                                                                    break;
                                                                                }

                                                                                buf.truncate(num_bytes);

                                                                                outgoing_messages_tx.send((
                                                                                    ClientPacket {
                                                                                        header: ClientHeader::Common(CommonHeader::Data),
                                                                                        slot,
                                                                                    }, buf.freeze()
                                                                                ))
                                                                                    .await
                                                                                    .map_err(|_|
                                                                                        io::Error::new(io::ErrorKind::Other, "channel closed")
                                                                                    )?;
                                                                            }

                                                                            Ok::<(), io::Error>(())
                                                                        }.fuse()
                                                                    };

                                                                    let forward_to_connection = async move {
                                                                        while let Some(buf) = tunnel_to_tcp_rx.next().await {
                                                                            to_tcp.write_all(&buf).await?;
                                                                        }
                                                                        Ok::<(), io::Error>(())
                                                                    }.fuse();

                                                                    let forwarders = {
                                                                        shadow_clone!(mut outgoing_messages_tx);

                                                                        async move {
                                                                            let res = tokio::select! {
                                                                                res = forward_to_tunnel => res,
                                                                                res = forward_to_connection => res,
                                                                            };

                                                                            info!("connection on slot {} closed {:?}", slot, res);

                                                                            if storage.lock().remove(&slot).is_some() {
                                                                                let _ = outgoing_messages_tx.send((
                                                                                    ClientPacket {
                                                                                        header: ClientHeader::Common(CommonHeader::Closed),
                                                                                        slot,
                                                                                    },
                                                                                    Default::default()
                                                                                )).await;
                                                                            };
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
                                                                payload.into(),
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
                                                                payload.into(),
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
                                                        payload.into(),
                                                    )).await?;
                                                }
                                                Ok::<_, crate::Error>(())
                                            }
                                        });
                                    }
                                    ConnectTarget::Internal(_) => {
                                        let (ch, mut tx, mut rx) = MixedChannel::new(16, 16);

                                        tokio::spawn({
                                            shadow_clone!(internal_server_connector);
                                            shadow_clone!(mut outgoing_messages_tx);
                                            shadow_clone!(storage);
                                            // shadow_clone!(client_config);

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

                                                storage.lock().insert(
                                                    slot,
                                                    Connection {
                                                        stop_handle,
                                                        tunnel_to_tcp_tx,
                                                    });

                                                tokio::spawn({
                                                    shadow_clone!(storage);
                                                    shadow_clone!(outgoing_messages_tx);

                                                    async move {
                                                        let forward_to_tunnel = {
                                                            shadow_clone!(mut outgoing_messages_tx);

                                                            async move {
                                                                while let Some(buf) = rx.next().await {
                                                                    outgoing_messages_tx.send((
                                                                        ClientPacket {
                                                                            header: ClientHeader::Common(CommonHeader::Data),
                                                                            slot,
                                                                        }, buf
                                                                    ))
                                                                        .await
                                                                        .map_err(|_|
                                                                            io::Error::new(io::ErrorKind::Other, "channel closed")
                                                                        )?;
                                                                }

                                                                Ok::<(), io::Error>(())
                                                            }.fuse()
                                                        };

                                                        let forward_to_internal_server = async move {
                                                            while let Some(buf) = tunnel_to_tcp_rx.next().await {
                                                                tx
                                                                    .send(buf)
                                                                    .await
                                                                    .map_err(|_| io::Error::new(io::ErrorKind::Other, "channel closed"))?
                                                            }
                                                            Ok::<(), io::Error>(())
                                                        }.fuse();

                                                        let forwarders = {
                                                            shadow_clone!(mut outgoing_messages_tx);

                                                            async move {
                                                                let res = tokio::select! {
                                                                    res = forward_to_tunnel => res,
                                                                    res = forward_to_internal_server => res,
                                                                };

                                                                info!("connection on slot {} closed {:?}", slot, res);

                                                                if storage.lock().remove(&slot).is_some() {
                                                                    let _ = outgoing_messages_tx.send((
                                                                        ClientPacket {
                                                                            header: ClientHeader::Common(CommonHeader::Closed),
                                                                            slot,
                                                                        },
                                                                        Default::default()
                                                                    )).await;
                                                                };
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

                                                Ok::<_, crate::Error>(())
                                            }
                                        });

                                        internal_server_connector.send(IoStream::new(ch)).await?;
                                    }
                                }
                            }
                            ServerHeader::Common(CommonHeader::Data) => {
                                let maybe_slot = storage
                                    .lock()
                                    .get(&slot)
                                    .cloned();
                                if let Some(mut slot) = maybe_slot {
                                    slot
                                        .tunnel_to_tcp_tx
                                        .send(payload.freeze())
                                        .await?;
                                } else {
                                    warn!("unknown slot {}, closing connection", slot);
                                    return Err(Error::UnknownSlot(slot));
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
                                } else {
                                    warn!("unknown slot {}, closing connection", slot);
                                    return Err(Error::UnknownSlot(slot));
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("error reading from sink {}", e);
                        break;
                    }
                }
            }

            Ok::<(), crate::Error>(())
        }
    }.fuse();

    let write_future = outgoing_messages_rx.map(Ok).forward(tx).fuse();

    let res = tokio::select! {
        r = read_future => r,
        r = write_future => r,
    };

    info!("tunnel closed with result {:?}", res);

    Ok(res?)
}

pub enum ServerConnection {
    Initiating(oneshot::Sender<Box<dyn Conn>>),
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

    fn take_initiating(self) -> Option<oneshot::Sender<Box<dyn Conn>>> {
        match self {
            ServerConnection::Initiating(tcp_stream) => Some(tcp_stream),
            ServerConnection::Established(_) => None,
        }
    }
}

pub trait Conn: AsyncRead + AsyncWrite + Send + Unpin + 'static {}

impl<T> Conn for T where T: AsyncRead + AsyncWrite + Send + Unpin + 'static {}

pub fn server_connection(
    transport: impl Stream<Item = Result<(ClientPacket, BytesMut), Error>>
        + Sink<(ServerPacket, Bytes), Error = Error>
        + Send
        + 'static,
) -> (
    impl Future<Output = Result<(), crate::Error>> + Send + 'static,
    crate::connector::Connector,
) {
    let storage = Arc::new(Mutex::new(HashMap::<Slot, ServerConnection>::new()));
    let (new_connection_req_tx, mut new_connection_req_rx) = mpsc::channel(2);

    let f = {
        async move {
            let slot_counter = AtomicU32::new(0);

            let (tx, mut rx) = transport.split();

            let (outgoing_messages_tx, outgoing_messages_rx) = mpsc::channel(16);

            let accept_connect_future = {
                shadow_clone!(mut outgoing_messages_tx);
                shadow_clone!(storage);

                #[allow(unreachable_code)]
                async move {
                    while let Some((ready_async_channel_tx, connect_target)) =
                        new_connection_req_rx.next().await
                    {
                        let slot: Slot = slot_counter
                            .fetch_add(1, Ordering::SeqCst)
                            .try_into()
                            .expect("slot overflow");

                        storage
                            .lock()
                            .insert(slot, ServerConnection::Initiating(ready_async_channel_tx));

                        outgoing_messages_tx
                            .send((
                                ServerPacket {
                                    header: ServerHeader::ConnectRequest,
                                    slot,
                                },
                                bincode::serialize(&ConnectRequestPayload {
                                    target: connect_target,
                                })
                                .unwrap()
                                .into(),
                            ))
                            .await
                            .map_err(|_| io::Error::new(io::ErrorKind::Other, "channel closed"))?;
                    }

                    Ok::<(), crate::Error>(())
                }
                .fuse()
            };

            let read_future = {
                shadow_clone!(storage);
                shadow_clone!(outgoing_messages_tx);

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
                                                    let (tunnel_to_tcp_tx, mut tunnel_to_channel) = mpsc::channel::<Bytes>(4);
                                                    let (stop_handle, mut stop_wait) = stop_handle::<()>();

                                                    let ready_connection_resolver = mem::replace(e.get_mut(), ServerConnection::Established(Connection {
                                                        stop_handle,
                                                        tunnel_to_tcp_tx,
                                                    })).take_initiating().unwrap();


                                                    let (channel, mut from_tunnel_tx, mut to_tunnel_rx) = to_async_rw(16, 16);

                                                    ready_connection_resolver.send(Box::new(channel))
                                                        .map_err(|_| io::Error::new(io::ErrorKind::Other, "tunnel closed: could ot sed to read_connection_resolver"))?;

                                                    tokio::spawn({
                                                        shadow_clone!(storage);
                                                        shadow_clone!(outgoing_messages_tx);

                                                        async move {
                                                            let forward_to_tunnel = {
                                                                shadow_clone!(outgoing_messages_tx);

                                                                async move {
                                                                    while let Some(buf) = to_tunnel_rx.next().await {
                                                                        shadow_clone!(mut outgoing_messages_tx);

                                                                        outgoing_messages_tx.send((
                                                                            ServerPacket {
                                                                                header: ServerHeader::Common(CommonHeader::Data),
                                                                                slot,
                                                                            }, buf
                                                                        )).await
                                                                            .map_err(|_|
                                                                                io::Error::new(io::ErrorKind::Other, "channel closed")
                                                                            )?;
                                                                    }

                                                                    Ok::<(), io::Error>(())
                                                                }.fuse()
                                                            };

                                                            let forward_to_connection = async move {
                                                                while let Some(buf) = tunnel_to_channel.next().await {
                                                                    from_tunnel_tx
                                                                        .send(buf)
                                                                        .await
                                                                        .map_err(|_| io::Error::new(io::ErrorKind::Other, "tunnel closed: could not send to from_tunnel_tx"))
                                                                        ?;
                                                                }
                                                                Ok::<(), io::Error>(())
                                                            }.fuse();

                                                            let forwarders = {
                                                                shadow_clone!(mut outgoing_messages_tx);

                                                                async move {
                                                                    let res = tokio::select! {
                                                                        res = forward_to_tunnel => res,
                                                                        res = forward_to_connection => res,
                                                                    };

                                                                    info!("connection on slot {} closed {:?}", slot, res);

                                                                    if storage.lock().remove(&slot).is_some() {
                                                                        outgoing_messages_tx.send((
                                                                            ServerPacket {
                                                                                header: ServerHeader::Common(CommonHeader::Closed),
                                                                                slot,
                                                                            },
                                                                            Default::default()
                                                                        )).await?;
                                                                    }

                                                                    res?;

                                                                    Ok::<(), crate::Error>(())
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
                                                warn!("unknown slot {}, closing connection", slot);
                                                return Err(Error::UnknownSlot(slot));
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
                                                warn!("unknown slot {}, closing connection", slot);
                                                return Err(Error::UnknownSlot(slot));
                                            }
                                        }
                                    }
                                    ClientHeader::Common(CommonHeader::Data) => {
                                        let maybe_connection = if let Some(slot) = storage
                                            .lock()
                                            .get(&slot)
                                        {
                                            slot.established().cloned()
                                        } else {
                                            warn!("unknown slot {}, closing connection", slot);
                                            return Err(Error::UnknownSlot(slot));
                                        };

                                        if let Some(conn) = maybe_connection {
                                            conn
                                                .tunnel_to_tcp_tx
                                                .clone()
                                                .send(payload.freeze())
                                                .await?;
                                        } else {
                                            warn!("received data while connection is not in established state");
                                            return Err(Error::CommandOnInitiatingConnection);
                                        }
                                    }
                                    ClientHeader::Common(CommonHeader::Closed) => {
                                        if let Some(slot) = storage
                                            .lock()
                                            .remove(&slot)
                                        {
                                            if let Some(conn) = slot.into_established() {
                                                conn.stop_handle.stop(());
                                            } else {
                                                warn!("closed command received during connnection initialization");
                                                return Err(Error::CommandOnInitiatingConnection);
                                            }
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("error reading from sink {}", e);
                                break;
                            }
                        }
                    }

                    Ok::<(), crate::Error>(())
                }
            }.fuse();

            let write_future = outgoing_messages_rx.map(Ok).forward(tx).fuse();

            pin_mut!(read_future);
            pin_mut!(write_future);
            pin_mut!(accept_connect_future);

            select_biased! {
                r = accept_connect_future => r,
                r = read_future => r,
                r = write_future => r,
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
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
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
    use std::net::{IpAddr, SocketAddr};

    use tokio::net::TcpListener;

    use crate::framed::{client_framed, server_framed};

    use super::*;
    use exogress_config_core::UpstreamDefinition;
    use exogress_config_core::{Config, ConfigVersion, Revision};
    use std::collections::BTreeMap;
    use tokio::runtime::Handle;

    #[tokio::test]
    async fn test_simple() {
        let buf1 = vec![1, 2, 3, 4, 5, 6];
        let buf2 = vec![7, 8, 9];
        let buf4 = vec![10, 11, 12];
        let buf1_3 = vec![65, 66, 67];

        let resolver = TokioAsyncResolver::from_system_conf(Handle::current())
            .await
            .unwrap();

        let mut server_side_listener =
            TcpListener::bind(&SocketAddr::new(IpAddr::from([127, 0, 0, 1]), 0))
                .await
                .unwrap();
        let server_side_socket = server_side_listener.local_addr().unwrap();

        let send_handle = tokio::spawn({
            shadow_clone!(buf1);
            shadow_clone!(buf2);
            shadow_clone!(buf4);
            shadow_clone!(buf1_3);

            async move {
                let (server_side, _remote_addr) = server_side_listener.accept().await.unwrap();

                let (bg, connector) = server_connection(server_framed(server_side));

                tokio::spawn(bg);

                {
                    let mut tunneled1 = connector
                        .get_connection("backend.upstream.exg".parse().unwrap())
                        .await
                        .unwrap();
                    tunneled1.write_all(&buf1).await.unwrap();
                }

                let mut tunneled2 = connector
                    .get_connection("backend.upstream.exg".parse().unwrap())
                    .await
                    .unwrap();
                tunneled2.write_all(&buf2).await.unwrap();

                {
                    let _tunneled3 = connector
                        .get_connection("backend.upstream.exg".parse().unwrap())
                        .await
                        .unwrap();
                }

                {
                    let mut tunneled4 = connector
                        .get_connection("backend.upstream.exg".parse().unwrap())
                        .await
                        .unwrap();
                    tunneled4.write_all(&buf4).await.unwrap();
                }

                let mut buf = vec![];
                tunneled2.read_to_end(&mut buf).await.unwrap();
                assert_eq!(buf, buf1_3);
            }
        });

        let mut tcp_tunneled = TcpListener::bind(&SocketAddr::new(IpAddr::from([127, 0, 0, 1]), 0))
            .await
            .unwrap();
        let connect_to_addr = tcp_tunneled.local_addr().unwrap();

        let mut upstreams = BTreeMap::new();
        upstreams.insert(
            "backend".parse().unwrap(),
            UpstreamDefinition::on_default_host(connect_to_addr.port()),
        );

        let client_config = Arc::new(RwLock::new(
            Config {
                version: ConfigVersion("0.0.1".parse().unwrap()),
                revision: Revision(1),
                name: "my-config".parse().unwrap(),
                exposes: Default::default(),
                upstreams,
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
        let mut read_buf4 = vec![];
        accepted_connection4.read_buf(&mut read_buf4).await.unwrap();

        accepted_connection2.write_all(&buf1_3).await.unwrap();

        mem::drop(accepted_connection2);

        assert_eq!(buf1, read_buf1);
        assert_eq!(buf2, read_buf2);
        assert_eq!(buf4, read_buf4);

        send_handle.await.unwrap();
    }
}
