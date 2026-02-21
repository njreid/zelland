use std::collections::BTreeMap;
use zellij_tile::prelude::*;

/// Background plugin — no visible pane. Subscribes to TabUpdate events and
/// responds to "list-tabs" pipe messages with a JSON array of tab info.
///
/// Install:
///   cargo build --target wasm32-wasip1 --release
///   scp target/wasm32-wasip1/release/zelland_tabs.wasm \
///       remote:~/.config/zellij/plugins/zelland-tabs.wasm
///
/// Usage from Zelland (via run_remote_command):
///   zellij -s <session> pipe \
///     --plugin file:~/.config/zellij/plugins/zelland-tabs.wasm \
///     --name list-tabs
///
/// Zellij launches the plugin on first use and reuses the running instance
/// on subsequent calls — no manual startup needed.
#[derive(Default)]
struct ZellandTabs {
    tabs: Vec<TabInfo>,
}

register_plugin!(ZellandTabs);

impl ZellijPlugin for ZellandTabs {
    fn load(&mut self, _config: BTreeMap<String, String>) {
        request_permission(&[PermissionType::ReadApplicationState]);
        subscribe(&[EventType::TabUpdate]);
    }

    fn update(&mut self, event: Event) -> bool {
        if let Event::TabUpdate(tabs) = event {
            self.tabs = tabs;
        }
        false // no re-render needed (background plugin)
    }

    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        if pipe_message.name == "list-tabs" {
            let payload: Vec<serde_json::Value> = self.tabs.iter().map(|t| {
                serde_json::json!({
                    "index": t.position,
                    "name": t.name,
                    "active": t.active,
                })
            }).collect();
            let json = serde_json::to_string(&payload)
                .unwrap_or_else(|_| "[]".into());
            // pipe_output_to_cli sends the string to the stdout of the
            // `zellij pipe` CLI invocation that triggered this message.
            pipe_output_to_cli(json);
        }
        // return false — don't block the pipe; output is already sent
        false
    }

    fn render(&mut self, _rows: usize, _cols: usize) {
        // Background plugin: no UI output
    }
}
