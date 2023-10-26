use clap::Parser;
use std::time::Duration;
use tokio::{
    signal::unix::{signal, SignalKind},
    sync::watch,
};
use tracing_appender::non_blocking::WorkerGuard;

mod tracing_panic_handler;

#[derive(clap::ValueEnum, Clone, Copy)]
pub enum LogFormat {
    Human,
    Json,
}

#[derive(clap::ValueEnum, Clone, Copy)]
pub enum Mode {
    Standard,
    ErrorAtRuntime,
    ErrorAtStartup,
    StdErrSpam,
}

#[derive(Parser, Clone)]
#[clap()]
pub struct ConfigOpts {
    #[arg(long, value_enum, default_value = "human")]
    pub log_format: LogFormat,
    #[arg(long, value_enum)]
    pub mode: Mode,
}

#[tokio::main]
async fn main() {
    let opts = ConfigOpts::parse();
    let _guard = init_tracing(opts.log_format);

    tracing::info!("Initializing!");

    // We need to block on this part to ensure that we immediately register these signals.
    // Otherwise if we included signal creation in the below spawned task we would be at the mercy of whenever tokio decides to start running the task.
    let mut interrupt = signal(SignalKind::interrupt()).unwrap();
    let mut terminate = signal(SignalKind::terminate()).unwrap();
    let (trigger_shutdown_tx, trigger_shutdown_rx) = watch::channel(false);
    tokio::spawn(async move {
        tokio::select! {
            _ = interrupt.recv() => {
                tracing::info!("received SIGINT");
            },
            _ = terminate.recv() => {
                tracing::info!("received SIGTERM");
            },
        };

        trigger_shutdown_tx.send(true).unwrap();
    });

    db_logic(trigger_shutdown_rx, opts.mode).await;
}

pub fn init_tracing(format: LogFormat) -> WorkerGuard {
    let (non_blocking, guard) = tracing_appender::non_blocking(std::io::stdout());

    let builder = tracing_subscriber::fmt().with_writer(non_blocking);

    match format {
        LogFormat::Json => builder.json().init(),
        LogFormat::Human => builder.init(),
    }

    // When in json mode we need to process panics as events instead of printing directly to stdout.
    // This is so that:
    // * We dont include invalid json in stdout
    // * panics can be received by whatever is processing the json events
    //
    // We dont do this for LogFormat::Human because the default panic messages are more readable for humans
    if let LogFormat::Json = format {
        crate::tracing_panic_handler::setup();
    }

    guard
}

async fn db_logic(mut trigger_shutdown_rx: watch::Receiver<bool>, mode: Mode) {
    if let Mode::ErrorAtStartup = mode {
        tracing::error!("An error occurs during startup");
    }

    tracing::info!("accepting inbound connections");

    let start = std::time::Instant::now();
    match mode {
        Mode::Standard | Mode::ErrorAtStartup => tracing::info!("some functionality occurs"),
        Mode::ErrorAtRuntime => tracing::error!("some error occurs"),
        Mode::StdErrSpam => {
            tracing::info!("some functionality occurs");
            loop {
                eprintln!("some library is spitting out nonsense you dont care about");
                tokio::task::yield_now().await;
                if start.elapsed() > Duration::from_secs(5) {
                    break;
                }
            }
            tracing::info!("other functionality occurs");
        }
    }

    trigger_shutdown_rx.changed().await.unwrap();
}
