use std::time::Duration;
use tokio::time::timeout;
use tokio_bin_process::event::Level;
use tokio_bin_process::event_matcher::EventMatcher;
use tokio_bin_process::BinProcess;

#[tokio::test(flavor = "multi_thread")]
async fn test_cooldb_by_binary_name() {
    // Setup cooldb
    let mut cooldb = cooldb(None).await;

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

    // Shutdown cooldb asserting that it encountered no errors
    cooldb.shutdown_and_then_consume_events(&[]).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_cooldb_by_binary_name_bench_profile() {
    // Setup cooldb with custom profile
    let mut cooldb = cooldb(Some("bench")).await;

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

    // Shutdown cooldb asserting that it encountered no errors
    cooldb.shutdown_and_then_consume_events(&[]).await;
}

async fn cooldb(profile: Option<&'static str>) -> BinProcess {
    let mut cooldb =
        BinProcess::start_binary_name("cooldb", "cooldb", &["--log-format", "json"], profile).await;

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
