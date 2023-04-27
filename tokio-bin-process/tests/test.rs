use std::time::Duration;
use tokio_bin_process::event::Level;
use tokio_bin_process::event_matcher::EventMatcher;
use tokio_bin_process::BinProcess;

#[tokio::test(flavor = "multi_thread")]
async fn test_cooldb() {
    // Setup cooldb
    let cooldb = cooldb().await;

    // Shutdown cooldb asserting that it encountered no errors
    cooldb.shutdown_and_then_consume_events(&[]).await;
}

async fn cooldb() -> BinProcess {
    let mut shotover =
        BinProcess::start_with_args("cooldb", "cooldb", &["--log-format", "json"]).await;

    tokio::time::timeout(
        Duration::from_secs(30),
        shotover.wait_for(
            &EventMatcher::new()
                .with_level(Level::Info)
                .with_target("cooldb")
                .with_message("accepting inbound connections"),
        ),
    )
    .await
    .unwrap();
    shotover
}
