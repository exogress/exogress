use async_io_stream::IoStream;
use bytes::Bytes;
use exogress_tunnel::MixedChannel;
use futures::channel::mpsc;
use futures::StreamExt;
use warp::Filter;

pub async fn internal_server(new_conn_rx: mpsc::Receiver<IoStream<MixedChannel, Bytes>>) {
    let h = warp::any()
        .and(warp::fs::dir("./static"))
        .with(warp::trace::request());
    warp::serve(h)
        .run_incoming(new_conn_rx.map(Ok::<_, anyhow::Error>))
        .await;
}

#[cfg(test)]
mod test {
    use super::*;
    use bytes::BytesMut;
    use futures::{SinkExt, StreamExt};
    use warp::Filter;

    #[tokio::test]
    async fn test_serve() {
        let (mut new_conn_tx, new_conn_rx) = mpsc::channel(1);

        const RESP: &str = "Hello World";

        tokio::spawn(async move {
            let h = warp::path("test")
                .map(|| RESP.to_string())
                .with(warp::trace::request());
            warp::serve(h)
                .run_incoming(new_conn_rx.map(|c| Ok::<_, anyhow::Error>(c)))
                .await;
        });

        let (channel, mut tx, mut rx) = MixedChannel::new(16, 16);

        new_conn_tx.send(IoStream::new(channel)).await.unwrap();

        static REQ: &str = "GET /test HTTP/1.1\n\n";

        tx.send(REQ.as_bytes().into()).await.unwrap();

        let mut read_bytes = BytesMut::new();

        loop {
            let res = rx.next().await.unwrap();

            read_bytes.extend_from_slice(&res);

            if let Ok(cur_string) = std::str::from_utf8(&read_bytes) {
                println!("{}", cur_string);
                if cur_string.contains(RESP) {
                    break;
                }
            }
        }
    }
}
