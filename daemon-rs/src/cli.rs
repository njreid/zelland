use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "zn", about = "zn CLI tool")]
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
    /// Send a notification to connected clients
    Notify {
        /// Notification title
        title: String,
        /// Notification body
        body: String,
        /// The specific question/blocker (optional)
        #[arg(long)]
        question: Option<String>,
        /// Source tag: user | claude_code | gemini_cli | zellij_plugin
        #[arg(long, default_value = "user")]
        source: String,
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
        Commands::Notify { title, body, question, source } => {
            let payload = serde_json::json!({
                "title": title,
                "body": body,
                "question": question,
                "session_name": std::env::var("ZELLIJ_SESSION_NAME").ok(),
                "tab_index": std::env::var("ZELLIJ_TAB_INDEX").ok()
                    .and_then(|v| v.parse::<u32>().ok()),
                "pane_id": std::env::var("ZELLIJ_PANE_ID").ok()
                    .and_then(|v| v.parse::<u32>().ok()),
                "source": source,
            });
            trigger_json(&base_url, "notify", &payload).await;
        }
    }
}

async fn trigger_json(base_url: &str, action: &str, payload: &serde_json::Value) {
    let url = format!("{}/api/v1/trigger/{}", base_url, action);
    let client = reqwest::Client::new();
    if let Err(e) = client.post(&url).json(payload).send().await {
        eprintln!("Failed to send notification: {}", e);
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
