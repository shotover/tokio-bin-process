// This test crate does not provide the full required interface for a binary under test by tokio-bin-process.
// We are only testing that a single, non-workspace, crate works fine.

use tokio::{
    signal::unix::{signal, SignalKind},
    sync::watch,
};

#[tokio::main]
async fn main() {
    // We need to block on this part to ensure that we immediately register these signals.
    // Otherwise if we included signal creation in the below spawned task we would be at the mercy of whenever tokio decides to start running the task.
    let mut interrupt = signal(SignalKind::interrupt()).unwrap();
    let mut terminate = signal(SignalKind::terminate()).unwrap();
    let (trigger_shutdown_tx, mut trigger_shutdown_rx) = watch::channel(false);
    tokio::spawn(async move {
        tokio::select! {
            _ = interrupt.recv() => {},
            _ = terminate.recv() => {},
        };

        trigger_shutdown_tx.send(true).unwrap();
    });

    trigger_shutdown_rx.changed().await.unwrap();

    // If we never output anything then its technically valid tracing json output
}
