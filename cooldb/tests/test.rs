use std::time::Duration;
use tokio::time::timeout;
use tokio_bin_process::bin_path;
use tokio_bin_process::event::Level;
use tokio_bin_process::event_matcher::EventMatcher;
use tokio_bin_process::BinProcess;

#[tokio::test(flavor = "multi_thread")]
async fn test_cooldb() {
    // Setup cooldb
    let mut cooldb = cooldb("standard").await;

    // Assert that some functionality occured.
    // Use a timeout to prevent the test hanging if no events occur.
    timeout(
        Duration::from_secs(5),
        cooldb.consume_events(1, Some(&[]), Some(&[])),
    )
    .await
    .unwrap()
    .assert_contains(
        &EventMatcher::new()
            .with_level(Level::Info)
            .with_message("some functionality occurs"),
    );

    cooldb
        .shutdown_and_then_consume_events(Some(&[]), Some(&[]))
        .await;
}

// Generally tokio-bin-process only cares about the contents of stdout.
// However if the application is spamming stderr then we need to ensure
// we are reading from stderr otherwise the application will deadlock.
#[tokio::test(flavor = "multi_thread")]
async fn test_cooldb_stderr_spam() {
    // Setup cooldb
    let mut cooldb = cooldb("std-err-spam").await;

    // Assert that some functionality occured.
    // Use a timeout to prevent the test hanging if no events occur.
    timeout(
        Duration::from_secs(5),
        cooldb.consume_events(1, Some(&[]), Some(&[])),
    )
    .await
    .unwrap()
    .assert_contains(
        &EventMatcher::new()
            .with_level(Level::Info)
            .with_message("some functionality occurs"),
    );

    timeout(
        Duration::from_secs(10),
        cooldb.consume_events(1, Some(&[]), Some(&[])),
    )
    .await
    .unwrap()
    .assert_contains(
        &EventMatcher::new()
            .with_level(Level::Info)
            .with_message("other functionality occurs"),
    );

    timeout(
        Duration::from_secs(10),
        cooldb.shutdown_and_then_consume_events(Some(&[]), Some(&[])),
    )
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
#[should_panic(expected = r#"some error occurs
Any ERROR or WARN events that occur in integration tests must be explicitly allowed by adding an appropriate EventMatcher to the method call."#)]
async fn test_cooldb_error_at_runtime() {
    let cooldb = cooldb("error-at-runtime").await;
    cooldb
        .shutdown_and_then_consume_events(Some(&[]), Some(&[]))
        .await;
}

#[tokio::test(flavor = "multi_thread")]
#[should_panic(expected = r#"An error occurs during startup
Any ERROR or WARN events that occur in integration tests must be explicitly allowed by adding an appropriate EventMatcher to the method call."#)]
async fn test_cooldb_error_at_startup() {
    let cooldb = cooldb("error-at-startup").await;
    cooldb
        .shutdown_and_then_consume_events(Some(&[]), Some(&[]))
        .await;
}

async fn cooldb(mode: &str) -> BinProcess {
    let mut cooldb = BinProcess::start_binary(
        bin_path!("cooldb"),
        "cooldb",
        &["--log-format", "json", "--mode", mode],
    )
    .await;

    timeout(
        Duration::from_secs(30),
        cooldb.wait_for(
            &EventMatcher::new()
                .with_level(Level::Info)
                .with_target("cooldb")
                .with_message("accepting inbound connections"),
            Some(&[]),
            Some(&[]),
        ),
    )
    .await
    .unwrap();
    cooldb
}
