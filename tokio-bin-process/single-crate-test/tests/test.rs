use std::time::Duration;
use tokio_bin_process::bin_path;
use tokio_bin_process::BinProcessBuilder;

#[tokio::test(flavor = "multi_thread")]
async fn test_cooldb_binary_name() {
    let process = BinProcessBuilder::from_cargo_name("single-crate-test".to_owned(), None)
        .with_args(vec!["--log-format".to_owned(), "json".to_owned()])
        .with_log_name(Some("cooldb".to_owned()))
        .start()
        .await;

    // give the process time to setup its sigint/sigterm handler
    tokio::time::sleep(Duration::from_secs(1)).await;

    process.shutdown_and_then_consume_events(&[]).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_cooldb_binary() {
    let process = BinProcessBuilder::from_path(bin_path!("single-crate-test"))
        .with_args(vec!["--log-format".to_owned(), "json".to_owned()])
        .with_log_name(Some("cooldb".to_owned()))
        .start()
        .await;

    // give the process time to setup its sigint/sigterm handler
    tokio::time::sleep(Duration::from_secs(1)).await;

    process.shutdown_and_then_consume_events(&[]).await;
}
