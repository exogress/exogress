use std::io;

use futures::channel::mpsc;
use futures::channel::mpsc::{Receiver, Sender};
use futures::ready;
use futures::stream::Fuse;
use futures::task::{Context, Poll};
use futures::{Sink, Stream, StreamExt};
use rw_stream_sink::RwStreamSink;
use tokio::macros::support::Pin;

// TODO: check how to properly handle close

pub struct MixedChannel {
    tx: mpsc::Sender<Vec<u8>>,
    rx: Fuse<mpsc::Receiver<Vec<u8>>>,
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
            rx: rx_receiver.fuse(),
        };

        (channel, tx_receiver, rx_sender)
    }
}

impl Stream for MixedChannel {
    type Item = Result<Vec<u8>, io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let res = ready!(Pin::new(&mut self.rx).poll_next(cx));

        Poll::Ready(res.map(Ok))
    }
}

impl Sink<Vec<u8>> for MixedChannel {
    type Error = io::Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.tx)
            .poll_ready(cx)
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "tunnel closed: could not poll mix"))
    }

    fn start_send(mut self: Pin<&mut Self>, item: Vec<u8>) -> Result<(), Self::Error> {
        Pin::new(&mut self.tx).start_send(item).map_err(|_| {
            io::Error::new(
                io::ErrorKind::Other,
                "tunnel closed: could not start_send to mix",
            )
        })
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.tx).poll_flush(cx).map_err(|_| {
            io::Error::new(
                io::ErrorKind::Other,
                "tunnel closed: could not poll_flush to mix",
            )
        })
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.tx).poll_close(cx).map_err(|_| {
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

    use futures::{SinkExt, StreamExt};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    use hyper::Uri;

    use crate::framed::{client_framed, server_framed};
    use crate::{client_listener, server_connection};

    use super::*;
    use exogress_config_core::{Config, ConfigVersion, Revision, UpstreamDefinition};
    use parking_lot::lock_api::RwLock;
    use std::collections::BTreeMap;
    use std::sync::Arc;
    use tokio::runtime::Handle;
    use trust_dns_resolver::TokioAsyncResolver;

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
        use bytes::buf::Buf;

        let resolver = TokioAsyncResolver::from_system_conf(Handle::current())
            .await
            .unwrap();

        let mut server_side_listener =
            TcpListener::bind(&SocketAddr::new(IpAddr::from([127u8, 0, 0, 1]), 0))
                .await
                .unwrap();

        let binded_to = server_side_listener.local_addr().unwrap();

        tokio::spawn({
            async move {
                let mut http_server =
                    TcpListener::bind(&SocketAddr::new(IpAddr::from([127u8, 0, 0, 1]), 0))
                        .await
                        .unwrap();

                let mut upstreams = BTreeMap::new();
                upstreams.insert(
                    "backend".parse().unwrap(),
                    UpstreamDefinition::on_default_host(http_server.local_addr().unwrap().port()),
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

                let tunnel = TcpStream::connect(&binded_to).await.unwrap();
                tokio::spawn(client_listener(
                    client_framed(tunnel),
                    client_config,
                    internal_server_connector,
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
        let mut body = hyper::body::aggregate(res).await.unwrap();

        assert_eq!(
            "Response body\r\n",
            String::from_utf8(body.to_bytes().to_vec()).unwrap()
        );
    }
}
