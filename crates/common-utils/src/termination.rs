use futures::stream::FuturesUnordered;
use futures::StreamExt;
use stop_handle::StopHandle;
use tokio::signal::unix::{signal, SignalKind};

async fn wait_signals(signal_kinds: &[SignalKind]) -> SignalKind {
    let mut f = FuturesUnordered::new();

    for kind in signal_kinds {
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

fn unix_termination_signals() -> [SignalKind; 3] {
    [
        SignalKind::interrupt(),
        SignalKind::quit(),
        SignalKind::terminate(),
    ]
}

pub async fn stop_signal_listener<R: StopSignal>(app_stop_handle: StopHandle<R>) {
    let kind = wait_signals(&unix_termination_signals()).await;

    info!("signal `{:?}` received", kind);

    app_stop_handle.stop(R::signal_received());
}

pub trait StopSignal {
    fn signal_received() -> Self;
}
