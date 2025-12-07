use clap::Parser;
use color_eyre::eyre;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::app::App;

pub mod api;
pub mod app;
pub mod buffers;
pub mod history;
pub mod query;
pub mod results;
pub mod widgets;

#[derive(Parser, Debug)]
#[command(name = "ghs")]
#[command(about = "GitHub Search TUI", long_about = None)]
struct Args {
    /// Path to the log file
    #[arg(long, default_value = ".ghs.log", env = "GHS_LOG")]
    log_file: Option<std::path::PathBuf>,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    let args = Args::parse();

    if let Some(log_path) = args.log_file {
        let log_dir = log_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        let log_file_name = log_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("ghs.log");

        std::fs::create_dir_all(log_dir)?;

        let file_appender = tracing_appender::rolling::never(log_dir, log_file_name);
        let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

        tracing_subscriber::registry()
            .with(
                tracing_subscriber::fmt::layer()
                    .with_writer(non_blocking)
                    .with_ansi(false)
                    .with_target(true)
                    .with_line_number(true),
            )
            .with(
                tracing_subscriber::EnvFilter::from_default_env()
                    .add_directive(tracing::Level::DEBUG.into()),
            )
            .init();
    }

    tracing::info!("Starting ghs");

    let terminal = ratatui::init();

    let result = App::run(terminal).await;

    ratatui::restore();

    if let Err(ref e) = result {
        tracing::error!("Application error: {}", e);
    }

    tracing::info!("Shutting down ghs");

    result
}
