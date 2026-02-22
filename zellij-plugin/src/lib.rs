use std::collections::BTreeMap;
use zellij_tile::prelude::*;

/// Increment this when the pipe protocol changes. Must match PLUGIN_VERSION in
/// src-tauri/src/lib.rs — Zelland uses this to decide whether to push an update.
const VERSION: u32 = 2;

/// Background plugin — no visible pane. Subscribes to TabUpdate events and
/// responds to "list-tabs" pipe messages with a versioned JSON object.
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
            let tabs: Vec<serde_json::Value> = self.tabs.iter().map(|t| {
                serde_json::json!({
                    "index": t.position,
                    "name": t.name,
                    "active": t.active,
                })
            }).collect();
            // Versioned envelope — Zelland checks "v" and pushes an updated
            // plugin binary if the version is below what it expects.
            let json = serde_json::json!({ "v": VERSION, "tabs": tabs }).to_string();
            cli_pipe_output(&pipe_message.name, &json);
            // Unblock the CLI so the `zellij pipe` process can exit.
            unblock_cli_pipe_input(&pipe_message.name);
        }
        false
    }

    fn render(&mut self, _rows: usize, _cols: usize) {
        // Background plugin: no UI output
    }
}
