use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use log::{debug, info, warn};
use serde::Deserialize;
use tauri::State;

use crate::keystore::KeyManager;
use crate::ssh::{SshConfig, SshManager};
use crate::ManagedKeyManager;

const HELPER_NAME: &str = "zlnd";
const HELPER_VERSION: &str = env!("CARGO_PKG_VERSION");
const HELPER_PORT_FILE: &str = ".local/state/zelland/zlnd.port";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RemotePlatform {
    LinuxX86_64,
    LinuxAarch64,
    MacosX86_64,
    MacosAarch64,
}

impl RemotePlatform {
    fn target_triple(self) -> &'static str {
        match self {
            Self::LinuxX86_64 => "x86_64-unknown-linux-gnu",
            Self::LinuxAarch64 => "aarch64-unknown-linux-gnu",
            Self::MacosX86_64 => "x86_64-apple-darwin",
            Self::MacosAarch64 => "aarch64-apple-darwin",
        }
    }

    fn artifact_name(self) -> String {
        format!("{HELPER_NAME}-{}", self.target_triple())
    }
}

#[derive(Deserialize)]
struct VersionResponse {
    version: String,
}

#[tauri::command]
pub async fn ensure_remote_helper(
    ssh_state: State<'_, SshManager>,
    key_state: State<'_, ManagedKeyManager>,
    config: SshConfig,
) -> Result<u16, String> {
    ensure_remote_helper_inner(&ssh_state, key_state.0.clone(), &config).await
}

pub async fn ensure_remote_helper_inner(
    ssh: &SshManager,
    key_manager: Arc<dyn KeyManager>,
    config: &SshConfig,
) -> Result<u16, String> {
    info!("Ensuring remote helper on {}", config.host);

    ssh.run_command(
        config.clone(),
        "mkdir -p ~/.local/state/zelland ~/bin".to_string(),
        key_manager.clone(),
    )
    .await?;

    let platform = detect_remote_platform(ssh, key_manager.clone(), config).await?;
    let remote_version = read_remote_helper_version(ssh, key_manager.clone(), config).await?;
    let mut replaced_binary = false;

    if remote_version.as_deref() != Some(HELPER_VERSION) {
        info!(
            "Installing helper {} on {} (current={:?}, target={})",
            platform.target_triple(),
            config.host,
            remote_version,
            HELPER_VERSION
        );
        let helper_bytes = load_helper_binary(platform).await?;
        let upload_path = "~/.local/state/zelland/zlnd.upload";
        ssh.upload_file(
            config.clone(),
            upload_path,
            &helper_bytes,
            key_manager.clone(),
        )
        .await?;
        ssh.run_command(
            config.clone(),
            format!(
                "chmod 755 {upload_path} && mv {upload_path} ~/bin/{HELPER_NAME}"
            ),
            key_manager.clone(),
        )
        .await?;
        replaced_binary = true;
    }

    // If we didn't replace the binary, check if the helper is already healthy
    // using whatever port it's currently listening on.
    if !replaced_binary {
        if let Some(port) = read_remote_port(ssh, key_manager.clone(), config).await {
            if helper_matches_expected_version(&helper_version_url(config, port)).await {
                return Ok(port);
            }
        }
    }

    // Start (or restart after binary replacement) the helper.
    ssh.run_command(
        config.clone(),
        start_command(replaced_binary),
        key_manager.clone(),
    )
    .await?;

    wait_for_helper_with_port(ssh, key_manager, config).await
}

async fn detect_remote_platform(
    ssh: &SshManager,
    key_manager: Arc<dyn KeyManager>,
    config: &SshConfig,
) -> Result<RemotePlatform, String> {
    let output = ssh
        .run_command(
            config.clone(),
            "uname -s && uname -m".to_string(),
            key_manager,
        )
        .await?;
    parse_remote_platform(&output)
}

async fn read_remote_helper_version(
    ssh: &SshManager,
    key_manager: Arc<dyn KeyManager>,
    config: &SshConfig,
) -> Result<Option<String>, String> {
    let output = ssh
        .run_command(
            config.clone(),
            "if [ -x \"$HOME/bin/zlnd\" ]; then \"$HOME/bin/zlnd\" --version; fi".to_string(),
            key_manager,
        )
        .await?;
    Ok(parse_helper_version(&output))
}

async fn read_remote_port(
    ssh: &SshManager,
    key_manager: Arc<dyn KeyManager>,
    config: &SshConfig,
) -> Option<u16> {
    let output = ssh
        .run_command(
            config.clone(),
            format!("cat ~/{HELPER_PORT_FILE} 2>/dev/null || true"),
            key_manager,
        )
        .await
        .ok()?;
    output.trim().parse().ok()
}

fn parse_remote_platform(output: &str) -> Result<RemotePlatform, String> {
    let mut lines = output.lines().map(str::trim).filter(|line| !line.is_empty());
    let os = lines.next().ok_or_else(|| "Remote platform probe returned no OS".to_string())?;
    let arch = lines.next().ok_or_else(|| "Remote platform probe returned no architecture".to_string())?;

    match (os, arch) {
        ("Linux", "x86_64") => Ok(RemotePlatform::LinuxX86_64),
        ("Linux", "aarch64") | ("Linux", "arm64") => Ok(RemotePlatform::LinuxAarch64),
        ("Darwin", "x86_64") => Ok(RemotePlatform::MacosX86_64),
        ("Darwin", "arm64") | ("Darwin", "aarch64") => Ok(RemotePlatform::MacosAarch64),
        _ => Err(format!(
            "Unsupported remote platform: {os} {arch} \
             (supported: Linux x86_64/aarch64, Darwin x86_64/arm64)"
        )),
    }
}

fn parse_helper_version(output: &str) -> Option<String> {
    output
        .split_whitespace()
        .find(|part| part.chars().next().is_some_and(|c| c.is_ascii_digit()))
        .map(|part| part.trim().trim_start_matches('v').to_string())
}

fn helper_version_url(config: &SshConfig, port: u16) -> String {
    format!("http://{}:{port}/api/v1/meta/version", config.host)
}

async fn helper_matches_expected_version(url: &str) -> bool {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(1))
        .build()
    {
        Ok(client) => client,
        Err(_) => return false,
    };

    match client.get(url).send().await {
        Ok(resp) if resp.status().is_success() => match resp.json::<VersionResponse>().await {
            Ok(payload) => payload.version == HELPER_VERSION,
            Err(_) => false,
        },
        _ => false,
    }
}

async fn wait_for_helper_with_port(
    ssh: &SshManager,
    key_manager: Arc<dyn KeyManager>,
    config: &SshConfig,
) -> Result<u16, String> {
    // Give the helper a moment to start before first probe.
    tokio::time::sleep(Duration::from_secs(1)).await;

    for _ in 0..10 {
        if let Some(port) = read_remote_port(ssh, key_manager.clone(), config).await {
            // Port file found — HTTP-poll for health at that port.
            let url = helper_version_url(config, port);
            for _ in 0..10 {
                if helper_matches_expected_version(&url).await {
                    return Ok(port);
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            return Err(format!(
                "Remote helper started on port {port} but did not become healthy — \
                 check ~/.local/state/zelland/zlnd.log on the remote host"
            ));
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    Err(
        "Remote helper did not write port file after 10 s — \
         check ~/.local/state/zelland/zlnd.log on the remote host"
            .to_string(),
    )
}

async fn load_helper_binary(platform: RemotePlatform) -> Result<Vec<u8>, String> {
    let cache_path = helper_cache_path(platform);
    if cache_path.exists() {
        return tokio::fs::read(&cache_path)
            .await
            .map_err(|e| format!("Failed to read cached helper {}: {}", cache_path.display(), e));
    }

    if let Some(local_path) = local_dev_helper_path(platform) {
        debug!("Using local helper binary at {}", local_path.display());
        let bytes = tokio::fs::read(&local_path)
            .await
            .map_err(|e| format!("Failed to read local helper {}: {}", local_path.display(), e))?;
        write_helper_cache(&cache_path, &bytes).await?;
        return Ok(bytes);
    }

    let url = format!(
        "https://github.com/{}/releases/download/v{HELPER_VERSION}/{}",
        helper_repository(),
        platform.artifact_name()
    );
    info!("Downloading helper from {url}");
    let response = reqwest::get(&url)
        .await
        .map_err(|e| format!("Failed to download helper from {url}: {}", e))?;
    if !response.status().is_success() {
        return Err(format!(
            "Failed to download helper binary {} (HTTP {}): \
             ensure release v{HELPER_VERSION} exists at {url}",
            platform.artifact_name(),
            response.status()
        ));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read helper asset bytes: {}", e))?
        .to_vec();
    write_helper_cache(&cache_path, &bytes).await?;
    Ok(bytes)
}

fn helper_repository() -> &'static str {
    option_env!("ZELLAND_HELPER_REPOSITORY").unwrap_or("njreid/zelland")
}

fn local_dev_helper_path(platform: RemotePlatform) -> Option<PathBuf> {
    let local_target = local_platform_target()?;
    if local_target != platform.target_triple() {
        return None;
    }

    let candidate = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("daemon-rs")
        .join("target")
        .join("release")
        .join(HELPER_NAME);

    candidate.exists().then_some(candidate)
}

fn local_platform_target() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Some("x86_64-unknown-linux-gnu"),
        ("linux", "aarch64") => Some("aarch64-unknown-linux-gnu"),
        ("macos", "x86_64") => Some("x86_64-apple-darwin"),
        ("macos", "aarch64") => Some("aarch64-apple-darwin"),
        _ => None,
    }
}

fn helper_cache_path(platform: RemotePlatform) -> PathBuf {
    std::env::temp_dir()
        .join("zelland-helper-cache")
        .join(HELPER_VERSION)
        .join(platform.artifact_name())
}

async fn write_helper_cache(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Failed to create helper cache dir {}: {}", parent.display(), e))?;
    }
    tokio::fs::write(path, bytes)
        .await
        .map_err(|e| format!("Failed to write helper cache {}: {}", path.display(), e))
}

fn start_command(force_restart: bool) -> String {
    let kill = if force_restart {
        "if command -v pkill >/dev/null 2>&1; then pkill -u \"$USER\" -f \"$HOME/bin/zlnd\" >/dev/null 2>&1 || true; fi; rm -f ~/.local/state/zelland/zlnd.port;"
    } else {
        ""
    };

    format!(
        "mkdir -p ~/.local/state/zelland ~/bin && {kill} \
         if command -v pgrep >/dev/null 2>&1 && pgrep -u \"$USER\" -f \"$HOME/bin/zlnd\" >/dev/null 2>&1; \
         then exit 0; fi && \
         nohup \"$HOME/bin/zlnd\" >\"$HOME/.local/state/zelland/zlnd.log\" 2>&1 </dev/null &"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_linux_x86_platform() {
        let platform = parse_remote_platform("Linux\nx86_64\n").unwrap();
        assert_eq!(platform, RemotePlatform::LinuxX86_64);
        assert_eq!(platform.artifact_name(), "zlnd-x86_64-unknown-linux-gnu");
    }

    #[test]
    fn parses_macos_arm_platform() {
        let platform = parse_remote_platform("Darwin\narm64\n").unwrap();
        assert_eq!(platform, RemotePlatform::MacosAarch64);
        assert_eq!(platform.target_triple(), "aarch64-apple-darwin");
    }

    #[test]
    fn parses_helper_version_from_clap_output() {
        assert_eq!(parse_helper_version("zlnd 0.1.0\n"), Some("0.1.0".to_string()));
    }

    #[test]
    fn start_command_restarts_when_requested() {
        let cmd = start_command(true);
        assert!(cmd.contains("pkill"));
        assert!(cmd.contains("nohup"));
        assert!(cmd.contains("zlnd.port"));
    }

    #[test]
    fn start_command_skips_kill_when_not_forced() {
        let cmd = start_command(false);
        assert!(!cmd.contains("pkill"));
        assert!(cmd.contains("nohup"));
    }
}
