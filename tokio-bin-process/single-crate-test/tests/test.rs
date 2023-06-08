use std::time::Duration;
use tokio_bin_process::BinProcess;

#[tokio::test(flavor = "multi_thread")]
async fn test_cooldb() {
    let process = BinProcess::start_with_args(
        "single-crate-test",
        "cooldb",
        &["--log-format", "json"],
        None,
    )
    .await;

    // give the process time to setup its sigint/sigterm handler
    tokio::time::sleep(Duration::from_secs(1)).await;

    process.shutdown_and_then_consume_events(&[]).await;
}
