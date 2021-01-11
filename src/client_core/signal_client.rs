use std::io;
use std::time::Duration;

use futures::channel::mpsc;
use futures::future::pending;
use futures::future::FutureExt;
use futures::{pin_mut, select_biased, stream::select, SinkExt, StreamExt};
use http::StatusCode;
use tokio::time::sleep;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, error, info, trace, warn};
use trust_dns_resolver::TokioAsyncResolver;
use url::Url;

use crate::common_utils::backoff::{Backoff, BackoffHandle};
use crate::config_core::ClientConfig;
use crate::signaling::{
    InstanceConfigMessage, SignalerHandshakeResponse, TunnelRequest, WsInstanceToCloudMessage,
};

use crate::client_core::health::UpstreamsHealth;
use crate::client_core::TunnelsStorage;
use crate::common_utils::jwt::JwtError;
use crate::entities::{InstanceId, SmolStr};
use crate::ws_client;
use crate::ws_client::connect_ws;
use parking_lot::{Mutex, RwLock};
use std::sync::Arc;
use tokio::sync::watch::Receiver;
use tokio_tungstenite::tungstenite::http::{Method, Request};
use tokio_tungstenite::tungstenite::protocol::CloseFrame;

#[derive(Debug, thiserror::Error)]
pub enum CloudConnectError {
    #[error("bad credentials")]
    Unauthorized,

    #[error("forbidden")]
    Forbidden,

    #[error("config conflict")]
    Conflict,
}

#[allow(clippy::too_many_arguments)]
pub async fn spawn(
    instance_id_storage: Arc<Mutex<Option<InstanceId>>>,
    current_config: Arc<RwLock<ClientConfig>>,
    mut config_rx: Receiver<ClientConfig>,
    tunnels: TunnelsStorage,
    url: Url,
    mut tx: mpsc::Sender<TunnelRequest>,
    mut rx: mpsc::Receiver<String>,
    upstream_healthcheck: UpstreamsHealth,
    authorization: SmolStr,
    backoff_min_duration: Duration,
    backoff_max_duration: Duration,
    maybe_identity: Option<Vec<u8>>,
    resolver: TokioAsyncResolver,
) -> Result<(), CloudConnectError> {
    let backoff = Backoff::new(backoff_min_duration, backoff_max_duration);

    pin_mut!(backoff);

    while let Some(backoff_handle) = backoff.next().await {
        info!("trying to establish connection to a signaler server");
        match do_conection(
            &instance_id_storage,
            &mut config_rx,
            &current_config,
            &authorization,
            backoff_handle,
            &url,
            &mut tx,
            &mut rx,
            &upstream_healthcheck,
            maybe_identity.clone(),
            &resolver,
        )
        .await
        {
            Ok(()) => {
                info!("Signal server connection closed. Will retry..");
            }
            Err(Error::Unauthorized) => {
                error!("Bad credentials");
                return Err(CloudConnectError::Unauthorized);
            }
            Err(Error::Forbidden) => {
                error!("access forbidden");
                return Err(CloudConnectError::Forbidden);
            }
            Err(Error::Conflict) => {
                error!("config conflict");
                return Err(CloudConnectError::Conflict);
            }
            Err(e) => {
                warn!("Presence error: {}", e);
            }
        }

        tunnels.clear();
    }

    Ok(())
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("unauthorized")]
    Unauthorized,

    #[error("forbidden")]
    Forbidden,

    #[error("conflict")]
    Conflict,

    #[error("bad status code: `{0}`")]
    BadStatus(StatusCode),

    #[error("unexpected ws message: `{0}`")]
    UnexpectedWsMessage(Message),

    #[error("io error: `{0}`")]
    Io(#[from] io::Error),

    #[error("websocket connect error: `{0}`")]
    ConnectError(#[from] ws_client::Error),

    #[error("ws error: `{0}`")]
    Websocket(#[from] tokio_tungstenite::tungstenite::error::Error),

    #[error("timeout waiting for pong")]
    Timeout(#[from] tokio::time::error::Elapsed),

    #[error("JSON error: `{0}`")]
    Json(#[from] serde_json::Error),

    #[error("JWT generation error: `{0}`")]
    Jwt(#[from] JwtError),

    #[error("handshake error: `{0}`")]
    HandshakeError(String),
}

#[allow(clippy::too_many_arguments)]
async fn do_conection(
    instance_id_storage: &Arc<Mutex<Option<InstanceId>>>,
    config_rx: &mut Receiver<ClientConfig>,
    current_config_storage: &RwLock<ClientConfig>,
    authorization: &str,
    backoff_handle: BackoffHandle,
    url: &Url,
    tx: &mut mpsc::Sender<TunnelRequest>,
    rx: &mut mpsc::Receiver<String>,
    upstream_healthcheck: &UpstreamsHealth,
    maybe_identity: Option<Vec<u8>>,
    resolver: &TokioAsyncResolver,
) -> Result<(), Error> {
    let current_config = current_config_storage.read().clone();

    async move {
        let initiate = {
            let current_config = current_config.clone();

            async move {
                let req = Request::builder()
                    .method(Method::GET)
                    .uri(url.to_string())
                    .header("Authorization", format!("Bearer {}", authorization))
                    .body(())
                    .unwrap();

                let (mut ws_stream, resp) =
                    connect_ws(req, resolver.clone(), maybe_identity).await?;

                match resp.status() {
                    StatusCode::UNAUTHORIZED => {
                        return Err(Error::Unauthorized);
                    }
                    StatusCode::FORBIDDEN => {
                        return Err(Error::Forbidden);
                    }
                    StatusCode::CONFLICT => {
                        return Err(Error::Conflict);
                    }
                    StatusCode::SWITCHING_PROTOCOLS => {}
                    _ => {
                        return Err(Error::BadStatus(resp.status()));
                    }
                }

                ws_stream
                    .send(Message::Text(
                        serde_json::to_string(&WsInstanceToCloudMessage::InstanceConfig(
                            InstanceConfigMessage {
                                config: current_config.clone(),
                            },
                        ))
                        .unwrap(),
                    ))
                    .await?;

                ws_stream
                    .send(Message::Text(
                        serde_json::to_string(&WsInstanceToCloudMessage::HealthState(
                            upstream_healthcheck.dump_health().await,
                        ))
                        .unwrap(),
                    ))
                    .await?;

                let instance_id = match ws_stream.next().await {
                    Some(Ok(Message::Text(response))) => {
                        match serde_json::from_str::<SignalerHandshakeResponse>(response.as_str())?
                        {
                            SignalerHandshakeResponse::Ok { instance_id } => instance_id,
                        }
                    }
                    r => {
                        let msg = r
                            .and_then(|r| match r {
                                Ok(msg) => match msg {
                                    Message::Close(Some(s)) => {
                                        serde_json::from_str::<serde_json::Value>(&s.reason)
                                            .ok()
                                            .and_then(|v| v.get("error").cloned())
                                            .and_then(|e| e.as_str().map(|s| s.to_string()))
                                    }
                                    _ => None,
                                },
                                Err(e) => Some(e.to_string()),
                            })
                            .unwrap_or_else(|| "no error provided".to_string());
                        return Err(Error::HandshakeError(msg));
                    }
                };

                Ok((ws_stream, current_config, instance_id))
            }
        };

        let (ws_stream, _current_config, instance_id) =
            timeout(Duration::from_secs(5), initiate).await??;

        let (mut ws_tx, mut ws_rx) = ws_stream.split();

        let (send_tx, mut send_rx) = mpsc::channel(1);
        let (mut recv_tx, mut recv_rx) = mpsc::channel::<String>(1);

        let (mut incoming_pongs_tx, mut incoming_pongs_rx) = mpsc::channel(2);

        info!("Connection to signal server established");

        let mut send_tx2 = send_tx.clone();

        let send_updated_config = async {
            while let Ok(()) = config_rx.changed().await {
                let config = config_rx.borrow().clone();
                info!("The new config uploaded");
                send_tx2
                    .send(Message::Text(
                        serde_json::to_string(&WsInstanceToCloudMessage::InstanceConfig(
                            InstanceConfigMessage { config },
                        ))
                        .unwrap(),
                    ))
                    .await?;
            }

            futures::future::pending::<Result<(), anyhow::Error>>().await
        }
        .fuse();

        // forward incoming messages, pings and poings to websocket
        let forward_sent_messages = {
            async {
                while let Some(msg) = select(&mut send_rx, rx.map(Message::Text)).next().await {
                    debug!("Send to WS: {:?}", msg);
                    if ws_tx.send(msg).await.is_err() {
                        break;
                    }
                }
            }
        }
        .fuse();

        // forward received messages to the code
        let forward_received_messages = {
            async {
                while let Some(msg) = recv_rx.next().await {
                    match serde_json::from_str(msg.as_str()) {
                        Ok(r) => {
                            if tx.send(r).await.is_err() {
                                break;
                            }
                        }
                        Err(e) => {
                            error!(
                                "Error parsing incoming signal JSON ({}): {}",
                                msg.as_str(),
                                e
                            );
                            return Err(e.into());
                        }
                    }
                }

                Ok(())
            }
        }
        .fuse();

        // after some time, reset backoff delay
        let accept_connection_with_delay = {
            async move {
                sleep(Duration::from_secs(5)).await;
                info!("mark connection as successful. reset backoff");
                backoff_handle.reset();
                pending::<()>().await
            }
        }
        .fuse();

        // handle messages received from Websocket
        let receiver = {
            let mut send_tx = send_tx.clone();

            async move {
                while let Some(incoming) = ws_rx.next().await {
                    match incoming {
                        Ok(Message::Ping(_)) => {
                            let _ = send_tx.send(Message::Pong(vec![])).await;
                        }
                        Ok(Message::Pong(v)) => {
                            let _ = incoming_pongs_tx.send(v).await;
                        }
                        Ok(Message::Text(s)) => {
                            let _ = recv_tx.send(s).await;
                        }
                        Ok(Message::Close(Some(CloseFrame { code, .. })))
                            if u16::from(code) == 4001 =>
                        {
                            return Err(Error::Unauthorized);
                        }
                        Ok(Message::Close(Some(CloseFrame { code, .. })))
                            if u16::from(code) == 4003 =>
                        {
                            return Err(Error::Forbidden);
                        }
                        Ok(Message::Close(Some(CloseFrame { code, .. })))
                            if u16::from(code) == 4009 =>
                        {
                            return Err(Error::Conflict);
                        }
                        Ok(msg) => {
                            return Err(Error::UnexpectedWsMessage(msg));
                        }
                        Err(e) => {
                            return Err(Error::Websocket(e));
                        }
                    }
                }

                Ok(())
            }
            .fuse()
        };

        // wait for incoming pongs. expect to receive at least one per 30 secs
        let pongs_acceptor = async move {
            while timeout(Duration::from_secs(30), incoming_pongs_rx.next())
                .await?
                .is_some()
            {
                trace!("ping received");
            }

            Ok(())
        }
        .fuse();

        // send ping each 15 seconds
        let pinger = {
            let mut send_tx = send_tx.clone();
            //

            async move {
                loop {
                    sleep(Duration::from_secs(15)).await;

                    // info!("Send ping");

                    if send_tx.send(Message::Ping(vec![])).await.is_err() {
                        break;
                    }
                }
            }
            .fuse()
        };

        pin_mut!(pongs_acceptor);
        pin_mut!(pinger);
        pin_mut!(accept_connection_with_delay);
        pin_mut!(receiver);
        pin_mut!(forward_sent_messages);
        pin_mut!(forward_received_messages);
        pin_mut!(send_updated_config);

        info!("set instance_id to {}", instance_id);
        *instance_id_storage.lock() = Some(instance_id);

        let r = select_biased! {
            res = receiver => {
                res
            }
            res = pongs_acceptor => {
                res
            }
            res = forward_received_messages => {
                res
            }
            _ = send_updated_config => {
                unreachable!()
            }
            _ = forward_sent_messages => {
                unreachable!()
            }
            _ = accept_connection_with_delay => {
                unreachable!()
            }
            _ = pinger => {
                unreachable!()
            }
        };

        *instance_id_storage.lock() = None;
        // TODO: FIXME: disconnect all tunnels

        info!("Connection to signal server closed. Clear instance_id");

        r
    }
    .await
}
