use smartstring::alias::String;
use std::io;
use std::time::Duration;

use futures::channel::mpsc;
use futures::future::pending;
use futures::future::FutureExt;
use futures::{pin_mut, select_biased, stream::select, SinkExt, StreamExt};
use http::StatusCode;
use rand::prelude::*;
use tokio::net::TcpStream;
use tokio::time::delay_for;
use tokio::time::timeout;
use tokio_rustls::{rustls::ClientConfig as RustlsClientConfig, TlsConnector};
use tokio_tungstenite::client_async;
use trust_dns_resolver::error::ResolveError;
use trust_dns_resolver::TokioAsyncResolver;
use tungstenite::Message;
use url::Url;
use webpki::DNSNameRef;

use exogress_common_utils::backoff::{Backoff, BackoffHandle};
use exogress_config_core::ClientConfig;
use exogress_signaling::TunnelRequest;

use exogress_common_utils::ws_client;
use exogress_common_utils::ws_client::connect_ws;
use exogress_entities::ClientId;
use jsonwebtoken::EncodingKey;
use parking_lot::RwLock;
use std::sync::Arc;
use tokio::sync::watch::Receiver;
use tokio_tungstenite::tungstenite::http::{Method, Request};
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tracing_futures::Instrument;

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    iss: String,
}

#[derive(Debug, thiserror::Error)]
pub enum CloudConnectError {
    #[error("bad credentials")]
    Unauthorized,

    #[error("forbidden")]
    Forbidden,

    #[error("config conflict")]
    Conflict,
}

pub async fn spawn(
    current_config: Arc<RwLock<ClientConfig>>,
    mut config_rx: Receiver<ClientConfig>,
    url: Url,
    mut tx: mpsc::Sender<TunnelRequest>,
    mut rx: mpsc::Receiver<String>,
    client_id: ClientId,
    jwt_encoding_key: EncodingKey,
    backoff_min_duration: Duration,
    backoff_max_duration: Duration,
    resolver: TokioAsyncResolver,
) -> Result<(), CloudConnectError> {
    let mut backoff = Backoff::new(backoff_min_duration, backoff_max_duration);
    let mut small_rng = SmallRng::from_entropy();

    while let Some(backoff_handle) = backoff.next().await {
        info!("trying to establish connection to a signaler server");
        match do_conection(
            &mut config_rx,
            &current_config,
            &client_id,
            &jwt_encoding_key,
            backoff_handle,
            &url,
            &mut tx,
            &mut rx,
            &resolver,
            &mut small_rng,
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
                warn!("Error on signal server connection: {}", e);
            }
        }
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
    Websocket(#[from] tungstenite::error::Error),

    #[error("timeout waiting for pong")]
    Timeout(#[from] tokio::time::Elapsed),

    #[error("JSON error: `{0}`")]
    Json(#[from] serde_json::Error),

    #[error("JWT generation error: `{0}`")]
    Jwt(#[from] jsonwebtoken::errors::Error),
}

async fn do_conection(
    config_rx: &mut Receiver<ClientConfig>,
    current_config_storage: &RwLock<ClientConfig>,
    client_id: &ClientId,
    jwt_encoding_key: &EncodingKey,
    backoff_handle: BackoffHandle,
    url: &Url,
    tx: &mut mpsc::Sender<TunnelRequest>,
    rx: &mut mpsc::Receiver<String>,
    resolver: &TokioAsyncResolver,
    small_rng: &mut SmallRng,
) -> Result<(), Error> {
    let current_config = current_config_storage.read().clone();

    async move {
        let initiate = {
            let current_config = current_config.clone();

            async move {
                let claims = Claims {
                    iss: client_id.to_string().into(),
                };

                let authorization = jsonwebtoken::encode(
                    &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::ES256),
                    &claims,
                    jwt_encoding_key,
                )?;

                let req = Request::builder()
                    .method(Method::GET)
                    .uri(url.to_string())
                    .header("Authorization", format!("Bearer {}", authorization))
                    .body(())
                    .unwrap();

                let (mut ws_stream, resp) = connect_ws(req, resolver.clone()).await?;

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
                        serde_json::to_string(current_config.as_ref()).unwrap(),
                    ))
                    .await?;

                Ok((ws_stream, current_config))
            }
        };

        let (ws_stream, _current_config) = timeout(Duration::from_secs(5), initiate).await??;

        let (mut ws_tx, mut ws_rx) = ws_stream.split();

        let (send_tx, mut send_rx) = mpsc::channel(1);
        let (mut recv_tx, mut recv_rx) = mpsc::channel::<String>(1);

        let (mut incoming_pongs_tx, mut incoming_pongs_rx) = mpsc::channel(2);

        info!("Connection to signal server established");

        let mut send_tx2 = send_tx.clone();

        let send_updated_config = async {
            while let Some(config) = config_rx.recv().await {
                info!("The new config uploaded");
                send_tx2
                    .send(Message::Text(
                        serde_json::to_string(config.as_ref()).unwrap(),
                    ))
                    .await?;
            }

            futures::future::pending::<Result<(), anyhow::Error>>().await
        }
        .fuse();

        // forward incoming messages, pings and poings to websocket
        let forward_sent_messages = {
            async {
                while let Some(msg) = select(&mut send_rx, rx.map(|r| Message::Text(r.to_string())))
                    .next()
                    .await
                {
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
                delay_for(Duration::from_secs(5)).await;
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
                            let _ = recv_tx.send(s.into()).await;
                        }
                        Ok(Message::Close(Some(CloseFrame { code, reason })))
                            if u16::from(code) == 4001 =>
                        {
                            return Err(Error::Unauthorized);
                        }
                        Ok(Message::Close(Some(CloseFrame { code, reason })))
                            if u16::from(code) == 4003 =>
                        {
                            return Err(Error::Forbidden);
                        }
                        Ok(Message::Close(Some(CloseFrame { code, reason })))
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
            while let Some(_) = timeout(Duration::from_secs(30), incoming_pongs_rx.next()).await? {
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
                    delay_for(Duration::from_secs(15)).await;

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
            res = send_updated_config => {
                unreachable!()
            }
            _ = forward_sent_messages => {
                unreachable!()
            }
            _ = accept_connection_with_delay => {
                unreachable!()
            }
            res = pinger => {
                unreachable!()
            }
        };

        info!("Connection to signal server closed");

        r
    }
    .await
}