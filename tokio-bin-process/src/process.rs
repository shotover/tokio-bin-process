use crate::event::{Event, Fields, Level};
use crate::event_matcher::{Count, EventMatcher, Events};
use anyhow::{anyhow, Context, Result};
use itertools::Itertools;
use nix::sys::signal::Signal;
use nix::unistd::Pid;
use nu_ansi_term::Color;
use once_cell::sync::Lazy;
use std::collections::HashSet;
use std::env;
use std::path::Path;
use std::process::Stdio;
use std::sync::Mutex;
use subprocess::{Exec, Redirection};
use tokio::sync::mpsc;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, ChildStdout, Command},
};
use tracing_subscriber::fmt::TestWriter;

// It is actually quite expensive to invoke `cargo build` even when there is nothing to build.
// On my machine, on a specific project, it takes 170ms.
// To avoid this cost for every call to start_with_args we use this global to keep track of which packages have been built by a BinProcess for the lifetime of the test run.
//
// Unfortunately this doesnt work when running each test in its own process. e.g. when using nextest
// But worst case it just unnecessarily reruns `cargo build`.
static BUILT_PACKAGES: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| Mutex::new(HashSet::new()));

/// A running process of a binary produced by a crate in the workspace
pub struct BinProcess {
    /// Always Some while BinProcess is owned
    pub child: Option<Child>,
    event_rx: mpsc::UnboundedReceiver<Event>,
}

impl Drop for BinProcess {
    fn drop(&mut self) {
        if self.child.is_some() && !std::thread::panicking() {
            panic!("Need to call either wait or shutdown_and_assert_success method on BinProcess before dropping it.");
        }
    }
}

fn setup_tracing_subscriber_for_test_logic() {
    tracing_subscriber::fmt()
        .with_writer(TestWriter::new())
        .with_env_filter("warn")
        .try_init()
        .ok();
}

impl BinProcess {
    /// Start the crate binary named `cargo_package_name` in a process and returns a BinProcess which can be used to interact with the process.
    /// The binary will be internally compiled by cargo if its not already, it will be compiled in a release/debug mode that matches the release/debug mode of the integration test.
    ///
    /// The `user_args` will be used as the args to the binary.
    /// The args should give the desired setup for the given integration test and should also enable the tracing json logger to stdout if that is not the default.
    /// All tracing events emitted by your binary in json over stdout will be processed by BinProcess and then emitted to the tests stdout in the default human readable tracing format.
    /// To ensure any WARN/ERROR's from your test logic are visible, BinProcess will setup its own subscriber that outputs to the tests stdout in the default human readable format.
    /// If you set your own subscriber before calling [`BinProcess::start_with_args`] that will take preference instead.
    ///
    /// Dropping the BinProcess will trigger a panic unless [`BinProcess::shutdown_and_then_consume_events`] or [`BinProcess::consume_remaining_events`] has been called.
    /// This is done to avoid missing important assertions run by those methods.
    pub async fn start_with_args(
        cargo_package_name: &str,
        log_name: &str,
        binary_args: &[&str],
    ) -> BinProcess {
        setup_tracing_subscriber_for_test_logic();

        let log_name = if log_name.len() > 10 {
            panic!("In order to line up in log outputs, argument log_name to BinProcess::start_with_args must be of length <= 10 but the value was: {log_name}");
        } else {
            format!("{log_name: <10}") //pads log_name up to 10 chars so that it lines up properly when included in log output.
        };

        // PROFILE is set in build.rs from PROFILE listed in https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-build-scripts
        let release = env!("PROFILE") == "release";

        let project_root = Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .ancestors()
            .nth(1)
            .unwrap()
            .to_path_buf();

        // First build the binary if its not yet built
        let mut built_packages = BUILT_PACKAGES.lock().unwrap();
        if !built_packages.contains(cargo_package_name) {
            let all_args = if release {
                vec!["build", "--all-features", "--release"]
            } else {
                vec!["build", "--all-features"]
            };
            run_command(&project_root, env!("CARGO"), &all_args).unwrap();
            built_packages.insert(cargo_package_name.to_owned());
        }

        // Now actually run the binary and keep hold of the child process
        let bin_path = project_root
            .join("target")
            .join(if release { "release" } else { "debug" })
            .join(cargo_package_name);
        let mut child = Some(
            Command::new(&bin_path)
                .args(binary_args)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true)
                .spawn()
                .context(format!("Failed to run {bin_path:?}"))
                .unwrap(),
        )
        .unwrap();

        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        let reader = BufReader::new(child.stdout.take().unwrap()).lines();
        tokio::spawn(async move {
            if let Err(err) = process_stdout_events(reader, &event_tx, log_name).await {
                // Because we are in a task, panicking is likely to be ignored.
                // Instead we generate a fake error event, which is possibly a bit confusing for the user but will at least cause the test to fail.
                event_tx
                    .send(Event {
                        timestamp: "".to_owned(),
                        level: Level::Error,
                        target: "tokio-bin-process".to_owned(),
                        fields: Fields {
                            message: err.to_string(),
                            fields: Default::default(),
                        },
                        span: Default::default(),
                        spans: Default::default(),
                    })
                    .ok();
            }
        });

        BinProcess {
            child: Some(child),
            event_rx,
        }
    }

    fn pid(&self) -> Pid {
        Pid::from_raw(self.child.as_ref().unwrap().id().unwrap() as i32)
    }

    pub fn signal(&self, signal: Signal) {
        nix::sys::signal::kill(self.pid(), signal).unwrap();
    }

    /// Waits for the `ready` EventMatcher to match on an incoming event.
    /// All events that were encountered while waiting are returned.
    pub async fn wait_for(&mut self, ready: &EventMatcher) -> Events {
        let mut events = vec![];
        while let Some(event) = self.event_rx.recv().await {
            let ready_match = ready.matches(&event);
            events.push(event);
            if ready_match {
                return Events { events };
            }
        }
        panic!("bin process shutdown before an event was found matching {ready:?}")
    }

    /// Await `event_count` messages to be emitted from the process.
    /// The emitted events are returned.
    pub async fn consume_events(
        &mut self,
        event_count: usize,
        expected_errors_and_warnings: &[EventMatcher],
    ) -> Events {
        let mut events = vec![];
        for _ in 0..event_count {
            match self.event_rx.recv().await {
                Some(event) => events.push(event),
                None => {
                    if events.is_empty() {
                        panic!("The process was terminated before the expected count of {event_count} events occured. No events received so far");
                    } else {
                        let events_received = events.iter().map(|x| format!("{x}")).join("\n");
                        panic!("The process was terminated before the expected count of {event_count} events occured. Events received so far:\n{events_received}");
                    }
                }
            }
        }
        BinProcess::assert_no_errors_or_warnings(&events, expected_errors_and_warnings);
        Events { events }
    }

    /// Issues sigterm to the process and then awaits its shutdown.
    /// All remaining events will be returned.
    pub async fn shutdown_and_then_consume_events(
        self,
        expected_errors_and_warnings: &[EventMatcher],
    ) -> Events {
        self.signal(nix::sys::signal::Signal::SIGTERM);
        self.consume_remaining_events(expected_errors_and_warnings)
            .await
    }

    /// prefer shutdown_and_then_consume_events.
    /// This method will not return until the process has terminated.
    /// It is useful when you need to test a shutdown method other than SIGTERM.
    pub async fn consume_remaining_events(
        mut self,
        expected_errors_and_warnings: &[EventMatcher],
    ) -> Events {
        let (events, status) = self
            .consume_remaining_events_inner(expected_errors_and_warnings)
            .await;

        if status != 0 {
            let events: String = events.events.iter().map(|x| format!("\n{x}")).collect();
            panic!("The bin process exited with {status} but expected 0 exit code (Success).\nevents:{events}");
        }

        events
    }

    /// Identical to consume_remaining_events but asserts that the process exited with failure code instead of success
    pub async fn consume_remaining_events_expect_failure(
        mut self,
        expected_errors_and_warnings: &[EventMatcher],
    ) -> Events {
        let (events, status) = self
            .consume_remaining_events_inner(expected_errors_and_warnings)
            .await;

        if status == 0 {
            let events: String = events.events.iter().map(|x| format!("\n{x}")).collect();
            panic!("The bin process exited with {status} but expected non 0 exit code (Failure).\nevents:{events}");
        }

        events
    }

    fn assert_no_errors_or_warnings(
        events: &[Event],
        expected_errors_and_warnings: &[EventMatcher],
    ) {
        let mut error_count = vec![0; expected_errors_and_warnings.len()];
        for event in events {
            if let Level::Error | Level::Warn = event.level {
                let mut matched = false;
                for (matcher, count) in expected_errors_and_warnings
                    .iter()
                    .zip(error_count.iter_mut())
                {
                    if matcher.matches(event) {
                        *count += 1;
                        matched = true;
                    }
                }
                if !matched {
                    panic!("Unexpected event {event}\nAny ERROR or WARN events that occur in integration tests must be explicitly allowed by adding an appropriate EventMatcher to the method call.")
                }
            }
        }

        // TODO: move into Events::contains
        for (matcher, count) in expected_errors_and_warnings.iter().zip(error_count.iter()) {
            match matcher.count {
                Count::Any => {}
                Count::Times(matcher_count) => {
                    if matcher_count != *count {
                        panic!("Expected to find matches for {matcher:?}, {matcher_count} times but actually matched {count} times")
                    }
                }
            }
        }
    }

    async fn consume_remaining_events_inner(
        &mut self,
        expected_errors_and_warnings: &[EventMatcher],
    ) -> (Events, i32) {
        // Take the child before we wait for the process to terminate.
        // This ensures that the drop bomb wont go off if the future is dropped partway through.
        // e.g. the user might have run BinProcess through `tokio::time::timeout`
        let child = self.child.take().unwrap();

        let mut events = vec![];
        while let Some(event) = self.event_rx.recv().await {
            events.push(event);
        }

        BinProcess::assert_no_errors_or_warnings(&events, expected_errors_and_warnings);

        use std::os::unix::process::ExitStatusExt;
        let output = child.wait_with_output().await.unwrap();
        let status = output.status.code().unwrap_or_else(|| {
            panic!(
                r#"Failed to get exit status.
The signal that killed the process was {:?}.
Possible caues:
* a SIGKILL was issued, something is going very wrong.
* a SIGINT or SIGTERM was issued but the aplications handler aborted without returning an exit value. (The default handler does this)
  If you are building a long running application you should handle SIGKILL and SIGTERM such that your application cleanly shutsdown and returns an exit value.
  Consider referring to how the tokio-bin-process example uses https://docs.rs/tokio/latest/tokio/signal/unix/struct.Signal.html
"#,
                output.status.signal()
            )
        });

        (Events { events }, status)
    }
}

async fn process_stdout_events(
    mut reader: tokio::io::Lines<BufReader<ChildStdout>>,
    event_tx: &mpsc::UnboundedSender<Event>,
    name: String,
) -> Result<()> {
    while let Some(line) = reader.next_line().await.context("An IO error occured while reading stdout from the application, I'm not actually sure when this happens?")? {
        let event = Event::from_json_str(&line).context(format!(
            "The application emitted a line that was not a valid event encoded in json: {}",
            line
        ))?;
        println!("{} {event}", Color::Default.dimmed().paint(&name));
        if event_tx.send(event).is_err() {
            // BinProcess is no longer interested in events
            return Ok(());
        }
    }
    Ok(())
}

/// Runs a command and returns the output as a string.
/// Both stderr and stdout are returned in the result.
fn run_command(working_dir: &Path, command: &str, args: &[&str]) -> Result<String> {
    let data = Exec::cmd(command)
        .args(args)
        .cwd(working_dir)
        .stdout(Redirection::Pipe)
        .stderr(Redirection::Merge)
        .capture()?;

    if data.exit_status.success() {
        Ok(data.stdout_str())
    } else {
        Err(anyhow!(
            "command {} {:?} exited with {:?} and output:\n{}",
            command,
            args,
            data.exit_status,
            data.stdout_str()
        ))
    }
}
