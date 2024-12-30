use std::time::Duration;
use tokio::time::timeout;
use tokio_bin_process::event::Level;
use tokio_bin_process::event_matcher::EventMatcher;
use tokio_bin_process::BinProcess;
use tokio_bin_process::{bin_path, BinProcessBuilder};

#[tokio::test(flavor = "multi_thread")]
async fn test_cooldb() {
    // Setup cooldb
    let mut cooldb = cooldb().await;

    // Assert that some functionality occured.
    // Use a timeout to prevent the test hanging if no events occur.
    timeout(Duration::from_secs(5), cooldb.consume_events(1, &[]))
        .await
        .unwrap()
        .assert_contains(
            &EventMatcher::new()
                .with_level(Level::Info)
                .with_message("some functionality occurs"),
        );

    cooldb.shutdown_and_then_consume_events(&[]).await;
}

async fn cooldb() -> BinProcess {
    let mut cooldb = BinProcessBuilder::from_path(bin_path!("cooldb"))
        .with_args(vec!["--log-format".to_owned(), "json".to_owned()])
        .start()
        .await;

    timeout(
        Duration::from_secs(30),
        cooldb.wait_for(
            &EventMatcher::new()
                .with_level(Level::Info)
                .with_target("cooldb")
                .with_message("accepting inbound connections"),
            &[],
        ),
    )
    .await
    .unwrap();
    cooldb
}
