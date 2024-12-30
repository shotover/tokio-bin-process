//! Allows your integration tests to run your application under a separate process and assert on [`tracing`](https://docs.rs/tracing) events.
//!
//! To achieve this, it locates or builds the application's executable,
//! runs it with `tracing` in JSON mode,
//! and then processes the JSON logs to both assert on and display in human readable form.
//!
//! It is a little opinionated and by default will fail the test when a `tracing` warning or error occurs.
//! However a specific warning or error can be allowed on a per test basis.
//!
//! Example usage for an imaginary database project named cooldb:
//!
//! ```rust
//! use tokio_bin_process::event::Level;
//! use tokio_bin_process::{BinProcess, BinProcessBuilder, bin_path};
//! use tokio_bin_process::event_matcher::EventMatcher;
//! use std::time::Duration;
//! use std::path::PathBuf;
//! # // hack to make the doc test compile
//! # macro_rules! bin_path {
//! #     ($bin_name:expr) => {
//! #         std::path::PathBuf::from("foo")
//! #     };
//! # }
//!
//! /// you'll want a helper like this as you'll be creating this in every integration test.
//! async fn cooldb_process() -> BinProcess {
//!     // start the process
//!     let mut process = BinProcessBuilder::from_path(
//!             // Locate the path to the cooldb binary from an integration test or benchmark
//!             bin_path!("cooldb")
//!         )
//!         .with_log_name(Some("cooldb1".to_owned())) // The name that BinProcess should prepend its forwarded logs with
//!         .with_env_vars(vec![
//!             // provide any custom env vars required
//!             ("FOO".to_owned(), "BAR".to_owned()),
//!             // tokio-bin-process relies on reading tracing json's output,
//!             // so configure the application to produce that
//!             ("COOLDB_LOG_FORMAT".to_owned(), "JSON".to_owned())
//!         ])
//!         .with_args(vec![
//!             // provide any custom CLI args required
//!             "--foo".to_owned(), "bar".to_owned(),
//!             // tokio-bin-process relies on reading tracing json's output,
//!             // so configure the application to produce that
//!             "--log-format".to_owned(), "json".to_owned()
//!         ])
//!         .start()
//!         .await;
//!
//!     // block asynchrounously until the application gives an event indicating that its ready
//!     tokio::time::timeout(
//!         Duration::from_secs(30),
//!         process.wait_for(
//!             &EventMatcher::new()
//!                 .with_level(Level::Info)
//!                 .with_target("cooldb")
//!                 .with_message("accepting inbound connections"),
//!             &[]
//!         ),
//!     )
//!     .await
//!     .unwrap();
//!     process
//! }
//!
//! #[tokio::test]
//! async fn test_some_functionality() {
//!     // start the db
//!     let cooldb = cooldb_process().await;
//!
//!     // connect to the db, do something and assert we get the expected result
//!     perform_test();
//!
//!     // Shutdown the DB, asserting that no warnings or errors occured,
//!     // but allow and expect a certain warning.
//!     // A drop bomb ensures that the test will fail if we forget to call this method.
//!     cooldb
//!         .shutdown_and_then_consume_events(&[
//!             EventMatcher::new()
//!                 .with_level(Level::Warn)
//!                 .with_target("cooldb::internal")
//!                 .with_message("The user did something silly that we want to warn about but is actually expected in this test case")
//!         ])
//!         .await;
//! }
//! ```
//!
//! When Cargo builds integration tests or benchmarks it provides a path to the binary under test.
//! We can make use of that for speed and robustness with [`BinProcessBuilder::from_path`].
//!
//! But that is not always flexible enough so as a fallback [`BinProcess`] can invoke Cargo again internally to ensure the binary we need is compiled via [`BinProcessBuilder::from_cargo_name`].
pub mod event;
pub mod event_matcher;
mod process;

pub use process::BinProcess;
pub use process::BinProcessBuilder;

/// When called from within an integration test or benchmark, returns the path to the binary with the specified crate name in the current package.
///
/// Whenever Cargo compiles a benchmark or integration test any binary crates in the same package will also be compiled.
/// This macro returns the path to one of those compiled binaries.
/// If no such binary exists then the macro will fail to compile.
///
/// For example:
/// There is a test at `test/tests.rs` for a binary crate with `name="foo"` in its `Cargo.toml` and with a `src/bin/bar.rs`.
/// Inside the test at `test/tests.rs`, both `bin_path!("foo")` and `bin_path!("bar")` would compile and return the path of their respective binaries.
#[macro_export]
macro_rules! bin_path {
    ($bin_name:expr) => {
        std::path::PathBuf::from(std::env!(concat!("CARGO_BIN_EXE_", $bin_name)))
    };
}
