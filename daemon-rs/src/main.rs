use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "zlnd", about = "zlnd companion daemon", version)]
struct Args {
    /// Path to KDL config file
    #[arg(long, short)]
    config: Option<PathBuf>,

    /// Port to listen on (overrides config)
    #[arg(long, short)]
    port: Option<u16>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    let mut config = match &args.config {
        Some(path) => zlnd_daemon::config::Config::load(path).unwrap_or_else(|e| {
            tracing::warn!("Failed to load config: {e}, using defaults");
            zlnd_daemon::config::Config::default()
        }),
        None => zlnd_daemon::config::Config::default(),
    };

    config.merge(args.port, None);

    if let Err(e) = zlnd_daemon::server::run(config).await {
        tracing::error!("Server error: {}", e);
        std::process::exit(1);
    }
}
