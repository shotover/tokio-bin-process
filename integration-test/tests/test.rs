use std::time::Duration;
use tokio::time::timeout;
use tokio_bin_process::bin_path;
use tokio_bin_process::event::Level;
use tokio_bin_process::event_matcher::EventMatcher;
use tokio_bin_process::BinProcess;

#[tokio::test(flavor = "multi_thread")]
async fn test_cooldb() {
    // Setup integration_test
    let mut integration_test = integration_test("standard").await;

    // Assert that some functionality occured.
    // Use a timeout to prevent the test hanging if no events occur.
    timeout(
        Duration::from_secs(5),
        integration_test.consume_events(1, &[]),
    )
    .await
    .unwrap()
    .assert_contains(
        &EventMatcher::new()
            .with_level(Level::Info)
            .with_message("some functionality occurs"),
    );

    integration_test.shutdown_and_then_consume_events(&[]).await;
}

// Generally tokio-bin-process only cares about the contents of stdout.
// However if the application is spamming stderr then we need to ensure
// we are reading from stderr otherwise the application will deadlock.
#[tokio::test(flavor = "multi_thread")]
async fn test_stderr_spam() {
    // Setup integration_test
    let mut integration_test = integration_test("std-err-spam").await;

    // Assert that some functionality occured.
    // Use a timeout to prevent the test hanging if no events occur.
    timeout(
        Duration::from_secs(5),
        integration_test.consume_events(1, &[]),
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
        integration_test.consume_events(1, &[]),
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
        integration_test.shutdown_and_then_consume_events(&[]),
    )
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
#[should_panic(expected = r#"some error occurs
Any ERROR or WARN events that occur in integration tests must be explicitly allowed by adding an appropriate EventMatcher to the method call."#)]
async fn test_error_at_runtime() {
    let integration_test = integration_test("error-at-runtime").await;
    integration_test.shutdown_and_then_consume_events(&[]).await;
}

#[tokio::test(flavor = "multi_thread")]
#[should_panic(expected = r#"An error occurs during startup
Any ERROR or WARN events that occur in integration tests must be explicitly allowed by adding an appropriate EventMatcher to the method call."#)]
async fn test_error_at_startup() {
    let integration_test = integration_test("error-at-startup").await;
    integration_test.shutdown_and_then_consume_events(&[]).await;
}

async fn integration_test(mode: &str) -> BinProcess {
    let mut integration_test = BinProcess::start_binary(
        bin_path!("integration-test"),
        "test",
        &["--log-format", "json", "--mode", mode],
    )
    .await;

    timeout(
        Duration::from_secs(30),
        integration_test.wait_for(
            &EventMatcher::new()
                .with_level(Level::Info)
                .with_target("integration_test")
                .with_message("accepting inbound connections"),
            &[],
        ),
    )
    .await
    .unwrap();
    integration_test
}
