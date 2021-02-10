use stop_handle::StopHandle;
#[cfg(unix)]
use tokio::signal::unix::{signal, SignalKind};
use tracing::info;

#[cfg(unix)]
async fn wait_unix_signal() -> SignalKind {
    use futures::{stream::FuturesUnordered, StreamExt};

    let unix_signals: [SignalKind; 3] = [
        SignalKind::interrupt(),
        SignalKind::quit(),
        SignalKind::terminate(),
    ];

    let mut f = FuturesUnordered::new();

    for kind in &unix_signals {
        let err_msg = format!("Could not listen for {:?}", kind);
        f.push(async move {
            signal(*kind).expect(&err_msg).recv().await;
            *kind
        });
    }

    f.next()
        .await
        .expect("unexpected termination of signal handler")
}

pub async fn stop_signal_listener<R: StopSignal>(app_stop_handle: StopHandle<R>) {
    #[cfg(unix)]
    {
        let kind = wait_unix_signal().await;
        info!("signal `{:?}` received", kind);
    }

    #[cfg(windows)]
    {
        tokio::signal::windows::ctrl_break()
            .expect("failed to set up ctrl_break handler")
            .recv()
            .await
            .expect("unexpected termination of signal handler");
    }

    app_stop_handle.stop(R::signal_received());
}

pub trait StopSignal {
    fn signal_received() -> Self;
}
