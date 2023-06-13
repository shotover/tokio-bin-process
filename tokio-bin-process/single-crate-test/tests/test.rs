use std::time::Duration;
use tokio_bin_process::bin_path;
use tokio_bin_process::BinProcess;

#[tokio::test(flavor = "multi_thread")]
async fn test_cooldb_crate_name() {
    let process = BinProcess::start_crate_name(
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

#[tokio::test(flavor = "multi_thread")]
async fn test_cooldb_binary() {
    let process = BinProcess::start_binary(
        bin_path!("single-crate-test"),
        "cooldb",
        &["--log-format", "json"],
    )
    .await;

    // give the process time to setup its sigint/sigterm handler
    tokio::time::sleep(Duration::from_secs(1)).await;

    process.shutdown_and_then_consume_events(&[]).await;
}
