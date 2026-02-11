use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "zelland", about = "zelland CLI tool")]
struct Args {
    #[command(subcommand)]
    command: Commands,

    /// Daemon port
    #[arg(long, short, default_value = "8083")]
    port: u16,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Display a file on the connected client
    Show {
        /// File path to show
        file: String,
    },
    /// Open a markdown file with annotation support
    Md {
        /// Markdown file path
        file: String,
    },
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let base_url = format!("http://127.0.0.1:{}", args.port);

    match args.command {
        Commands::Show { file } => {
            trigger(&base_url, "show", &file).await;
        }
        Commands::Md { file } => {
            trigger(&base_url, "md", &file).await;
        }
    }
}

async fn trigger(base_url: &str, action: &str, file: &str) {
    let url = format!("{}/api/v1/trigger/{}", base_url, action);
    let abs_path = std::fs::canonicalize(file).unwrap_or_else(|_| {
        eprintln!("File not found: {}", file);
        std::process::exit(1);
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .json(&serde_json::json!({ "path": abs_path.to_string_lossy() }))
        .send()
        .await;

    match resp {
        Ok(r) if r.status().is_success() => {
            println!("OK: {} {}", action, abs_path.display());
        }
        Ok(r) => {
            eprintln!("Error: {} {}", r.status(), r.text().await.unwrap_or_default());
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Failed to connect to daemon: {}", e);
            std::process::exit(1);
        }
    }
}
