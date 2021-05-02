use std::io;

use futures::{
    channel::{
        mpsc,
        mpsc::{Receiver, Sender},
    },
    ready,
    stream::Fuse,
    task::{Context, Poll, Waker},
    Sink, Stream, StreamExt,
};
use rw_stream_sink::RwStreamSink;
use tokio::macros::support::Pin;

pub struct MixedChannel {
    tx: mpsc::Sender<Vec<u8>>,
    rx: Option<Fuse<mpsc::Receiver<Vec<u8>>>>,
    sink_waker: Option<Waker>,
    stream_waker: Option<Waker>,
}

impl MixedChannel {
    pub fn new(
        buf_sender: usize,
        buf_receiver: usize,
    ) -> (Self, Sender<Vec<u8>>, Receiver<Vec<u8>>) {
        let (tx_sender, rx_sender) = mpsc::channel(buf_sender);
        let (tx_receiver, rx_receiver) = mpsc::channel(buf_receiver);

        let channel = MixedChannel {
            tx: tx_sender,
            rx: Some(rx_receiver.fuse()),
            sink_waker: None,
            stream_waker: None,
        };

        (channel, tx_receiver, rx_sender)
    }
}

impl Stream for MixedChannel {
    type Item = Result<Vec<u8>, io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Some(rx) = &mut self.rx {
            let res = ready!(Pin::new(rx).poll_next(cx));
            self.stream_waker = Some(cx.waker().clone());

            if let Some(r) = res {
                Poll::Ready(Some(Ok(r)))
            } else {
                self.tx.close_channel();
                if let Some(w) = self.sink_waker.as_ref() {
                    w.wake_by_ref()
                }
                Poll::Ready(None)
            }
        } else {
            Poll::Ready(None)
        }
    }
}

impl MixedChannel {
    fn close_stream(&mut self) {
        let _ = self.rx.take();
        if let Some(w) = self.stream_waker.as_ref() {
            w.wake_by_ref()
        };
    }
}

impl Sink<Vec<u8>> for MixedChannel {
    type Error = io::Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.sink_waker = Some(cx.waker().clone());

        Pin::new(&mut self.tx).poll_ready(cx).map_err(|_| {
            self.close_stream();
            io::Error::new(io::ErrorKind::Other, "tunnel closed: could not poll mix")
        })
    }

    fn start_send(mut self: Pin<&mut Self>, item: Vec<u8>) -> Result<(), Self::Error> {
        Pin::new(&mut self.tx).start_send(item).map_err(|_| {
            self.close_stream();
            io::Error::new(
                io::ErrorKind::Other,
                "tunnel closed: could not start_send to mix",
            )
        })
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.sink_waker = Some(cx.waker().clone());

        Pin::new(&mut self.tx).poll_flush(cx).map_err(|_| {
            self.close_stream();
            io::Error::new(
                io::ErrorKind::Other,
                "tunnel closed: could not poll_flush to mix",
            )
        })
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.sink_waker = Some(cx.waker().clone());

        Pin::new(&mut self.tx)
            .poll_close(cx)
            .map(|r| {
                self.close_stream();
                r
            })
            .map_err(|_| {
                self.close_stream();
                io::Error::new(
                    io::ErrorKind::Other,
                    "tunnel closed: could not poll_close to mix",
                )
            })
    }
}

pub fn to_async_rw(
    buf_sender: usize,
    buf_receiver: usize,
) -> (
    RwStreamSink<MixedChannel>,
    mpsc::Sender<Vec<u8>>,
    mpsc::Receiver<Vec<u8>>,
) {
    let (mixed, tx, rx) = MixedChannel::new(buf_sender, buf_receiver);
    (RwStreamSink::new(mixed), tx, rx)
}

#[cfg(test)]
mod test {
    use std::net::{IpAddr, SocketAddr};

    use futures::{SinkExt, StreamExt, TryStreamExt};
    use tokio::net::{TcpListener, TcpStream};

    use hyper::Uri;

    use crate::tunnel::{
        client_listener,
        framed::{client_framed, server_framed},
        server_connection,
    };

    use super::*;
    use crate::config_core::{
        refinable::Refinable, ClientConfig, ClientConfigRevision, UpstreamDefinition,
        CURRENT_VERSION,
    };
    use bytes::Bytes;
    use futures::{AsyncReadExt, AsyncWriteExt};
    use parking_lot::lock_api::RwLock;
    use std::{collections::BTreeMap, mem, sync::Arc};
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    use trust_dns_resolver::{TokioAsyncResolver, TokioHandle};

    #[tokio::test]
    async fn test_mixed_channel_close_stream_sink() {
        let (rw, mut ch_tx, mut ch_rx) = to_async_rw(2, 2);

        #[allow(unreachable_code)]
        let sender = tokio::spawn(async move {
            loop {
                ch_tx.send(vec![1]).await?;
            }

            Ok::<_, mpsc::SendError>(())
        });
        let receiver = tokio::spawn(async move { ch_rx.next().await });

        mem::drop(rw);

        assert!(receiver.await.unwrap().is_none());
        assert!(sender.await.unwrap().is_err());
    }

    #[tokio::test]
    async fn test_mixed_channel_close_tx() {
        let (rw, ch_tx, _ch_rx) = to_async_rw(2, 2);

        let (mut r, mut w) = rw.split();

        #[allow(unreachable_code)]
        let sender = tokio::spawn(async move {
            loop {
                w.write_all(&[1]).await?;
            }

            Ok::<_, io::Error>(())
        });
        let receiver = tokio::spawn(async move {
            let mut v = Vec::new();

            r.read_to_end(&mut v).await
        });

        mem::drop(ch_tx);

        assert!(matches!(receiver.await, Ok(Ok(0))));
        assert!(matches!(sender.await, Ok(Err(_))));
    }

    #[tokio::test]
    async fn test_mixed_channel_close_rx() {
        let (rw, _ch_tx, ch_rx) = to_async_rw(2, 2);

        let (mut r, mut w) = rw.split();

        #[allow(unreachable_code)]
        let sender = tokio::spawn(async move {
            loop {
                w.write_all(&[1]).await?;
            }

            Ok::<_, io::Error>(())
        });
        let receiver = tokio::spawn(async move {
            let mut v = Vec::new();

            r.read_to_end(&mut v).await
        });

        mem::drop(ch_rx);

        assert!(matches!(receiver.await, Ok(Ok(0))));
        assert!(matches!(sender.await, Ok(Err(_))));
    }

    #[tokio::test]
    async fn test_channel() {
        let (mut rw, mut ch1_tx, mut ch2_rx) = to_async_rw(2, 2);

        ch1_tx.send(vec![1, 2, 3].into()).await.unwrap();
        ch1_tx.send(vec![4, 5].into()).await.unwrap();

        let mut buf = vec![0u8; 5];
        rw.read_exact(&mut buf).await.unwrap();

        assert_eq!(vec![1, 2, 3, 4, 5], buf);

        rw.write_all(&mut [1u8, 2, 3, 4, 5, 6]).await.unwrap();
        rw.write_all(&mut [7u8]).await.unwrap();

        assert_eq!(
            Bytes::from_static(&[1u8, 2, 3, 4, 5, 6]),
            ch2_rx.next().await.unwrap()
        );
        assert_eq!(Bytes::from_static(&[7u8]), ch2_rx.next().await.unwrap());
    }

    #[tokio::test]
    async fn test_service_fn() {
        let resolver = TokioAsyncResolver::from_system_conf(TokioHandle).unwrap();

        let server_side_listener =
            TcpListener::bind(&SocketAddr::new(IpAddr::from([127u8, 0, 0, 1]), 0))
                .await
                .unwrap();

        let binded_to = server_side_listener.local_addr().unwrap();

        tokio::spawn({
            async move {
                let http_server =
                    TcpListener::bind(&SocketAddr::new(IpAddr::from([127u8, 0, 0, 1]), 0))
                        .await
                        .unwrap();

                let mut upstreams = BTreeMap::new();
                upstreams.insert(
                    "backend".parse().unwrap(),
                    UpstreamDefinition::on_default_host(http_server.local_addr().unwrap().port()),
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
                            rescues: vec![],
                        },
                    }
                    .into(),
                ));

                let (internal_server_connector, _new_conn_rx) = mpsc::channel(1);

                let tunnel = TcpStream::connect(&binded_to).await.unwrap();
                tokio::spawn(client_listener(
                    client_framed(tunnel),
                    client_config,
                    internal_server_connector,
                    &None,
                    resolver,
                ));

                let response =
                    b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\nResponse body\r\n";

                let (mut conn, _) = http_server.accept().await.unwrap();

                let mut b = vec![];
                conn.read_buf(&mut b).await.unwrap();
                conn.write_all(response).await.unwrap();
            }
        });

        let (server_side, _remote_addr) = server_side_listener.accept().await.unwrap();

        let (bg, connector) = server_connection(server_framed(server_side));

        tokio::spawn(bg);

        let client = hyper::Client::builder().build::<_, hyper::Body>(connector);

        let res = client
            .get(Uri::from_static("http://backend.upstream.exg/test"))
            .await
            .unwrap();
        let body = res
            .into_body()
            .try_fold(Vec::new(), |mut data, chunk| async move {
                data.extend_from_slice(&chunk);
                Ok(data)
            })
            .await
            .unwrap();

        assert_eq!(
            "Response body\r\n",
            std::str::from_utf8(body.as_ref()).unwrap()
        );
    }
}
