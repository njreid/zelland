# Design: Push Notifications with Deep-Link Navigation

## Overview

zelland already has a near-complete notification pipeline.  This document describes
the minimal additions needed to make it fully useful: richer context in the proto
message, a `zn notify` CLI subcommand, deep-link navigation on tap, and hook scripts
that fire when an AI agent (Claude Code, Gemini CLI) is waiting for user input.

---

## 1. Current State

The end-to-end path already works:

```
zn show <file>
  └─ POST /api/v1/trigger/show   (loopback-only)
       └─ ClientRegistry::broadcast()  (WebSocket fan-out)
            └─ DaemonManager::recv loop  (Tauri client)
                 └─ app_handle.notification().show()
```

The `Notification` proto message already routes through this same path in
`daemon.rs:75-81`:

```rust
Some(Payload::Notification(notif)) => {
    let _ = app_handle.notification()
        .builder()
        .title(notif.title.clone())
        .body(notif.body.clone())
        .show();
}
```

**What is missing:**

| Gap | Impact |
|---|---|
| `Notification` proto has only `title` + `body` | No Zellij session/pane context → can't deep-link |
| No `trigger_notify` daemon handler | No loopback HTTP endpoint to fire a notification |
| `zn` CLI has no `notify` subcommand | No shell-callable tool for agents / scripts |
| Notification tap does nothing useful | App opens at wrong session or does nothing |
| KDE: no "Focus" action button | User has to alt-tab manually |

---

## 2. Proto Changes

Extend `proto/zelland.proto`.  The existing `Notification` message gets context
fields via a nested `NavigationTarget`.  Keeping backward compatibility: a
notification without a target still shows title + body.

```proto
// Zellij coordinate system — all three fields come from env vars that Zellij
// injects into every pane: $ZELLIJ_SESSION_NAME, $ZELLIJ_TAB_INDEX, $ZELLIJ_PANE_ID
message NavigationTarget {
  string session_name = 1;
  uint32 tab_index = 2;
  uint32 pane_id = 3;
}

// Source of the notification — used for icon/badge selection on client
enum NotificationSource {
  USER = 0;
  CLAUDE_CODE = 1;
  GEMINI_CLI = 2;
  ZELLIJ_PLUGIN = 3;
}

message Notification {
  string title = 1;
  string body = 2;
  // Optional: where to navigate when user taps the notification
  NavigationTarget target = 3;
  // Optional: the verbatim question the agent is asking
  string question = 4;
  NotificationSource source = 5;
}
```

---

## 3. Daemon Changes

### 3.1 New trigger handler: `trigger_notify`

Add `daemon-rs/src/handlers/trigger.rs` → `trigger_notify`:

```rust
// POST /api/v1/trigger/notify   (loopback-only — same guard as show/md)
#[derive(Deserialize)]
pub struct NotifyRequest {
    pub title: String,
    pub body: String,
    // Optional Zellij context
    pub session_name: Option<String>,
    pub tab_index: Option<u32>,
    pub pane_id: Option<u32>,
    // Optional agent question text
    pub question: Option<String>,
    // "user" | "claude_code" | "gemini_cli" | "zellij_plugin"
    pub source: Option<String>,
}

pub async fn trigger_notify(
    State(state): State<AppState>,
    Json(req): Json<NotifyRequest>,
) -> StatusCode {
    let target = match &req.session_name {
        Some(s) => Some(NavigationTarget {
            session_name: s.clone(),
            tab_index: req.tab_index.unwrap_or(0),
            pane_id: req.pane_id.unwrap_or(0),
        }),
        None => None,
    };
    let source = match req.source.as_deref() {
        Some("claude_code")   => NotificationSource::ClaudeCode,
        Some("gemini_cli")    => NotificationSource::GeminiCli,
        Some("zellij_plugin") => NotificationSource::ZellijPlugin,
        _                     => NotificationSource::User,
    } as i32;

    let envelope = Envelope {
        payload: Some(Payload::Notification(Notification {
            title: req.title,
            body: req.body,
            target,
            question: req.question.unwrap_or_default(),
            source,
        })),
    };
    state.registry.broadcast(&envelope);
    StatusCode::OK
}
```

Register in `server.rs`:

```rust
.route("/api/v1/trigger/notify", post(handlers::trigger::trigger_notify))
// (inside the trigger_routes block — inherits loopback_guard)
```

### 3.2 `zn notify` CLI subcommand

Extend `daemon-rs/src/cli.rs`:

```rust
Commands::Notify {
    title,
    body,
    question,
    source,
} => {
    let session_name = std::env::var("ZELLIJ_SESSION_NAME").ok();
    let tab_index = std::env::var("ZELLIJ_TAB_INDEX")
        .ok().and_then(|v| v.parse::<u32>().ok());
    let pane_id = std::env::var("ZELLIJ_PANE_ID")
        .ok().and_then(|v| v.parse::<u32>().ok());

    let payload = serde_json::json!({
        "title": title,
        "body": body,
        "question": question,
        "session_name": session_name,
        "tab_index": tab_index,
        "pane_id": pane_id,
        "source": source.unwrap_or("user".into()),
    });
    let client = reqwest::Client::new();
    client.post(format!("{}/api/v1/trigger/notify", base_url))
        .json(&payload).send().await.ok();
}
```

Usage from a terminal running inside Zellij:

```bash
# Simple
zn notify "Build done" "cargo build finished successfully"

# With source tag (used by agent hooks)
zn notify "Claude needs input" \
  "Should I overwrite the existing migration?" \
  --source claude_code \
  --question "Should I overwrite the existing migration?"

# $ZELLIJ_SESSION_NAME / _TAB_INDEX / _PANE_ID are read automatically
```

---

## 4. Tauri Client Changes

### 4.1 Deep-link context on Android

When `DaemonManager` receives a `Notification` with a `target`, attach the
navigation context to the system notification so Android can restore it on tap.

```rust
Some(Payload::Notification(notif)) => {
    let mut builder = app_handle.notification()
        .builder()
        .title(&notif.title)
        .body(&notif.body);

    // Encode the target as a JSON action payload stored in the notification
    if let Some(ref t) = notif.target {
        let nav = serde_json::json!({
            "session_name": t.session_name,
            "tab_index": t.tab_index,
            "pane_id": t.pane_id,
        });
        // tauri-plugin-notification supports action payloads
        builder = builder.action("navigate", &nav.to_string());
    }

    let _ = builder.show();

    // Also emit a Tauri event so the frontend can show an in-app banner
    // if the app is already focused
    let _ = app_handle.emit("agent-notification", notif);
}
```

### 4.2 Notification tap handler

In `src-tauri/src/intent.rs` (Android) and `src-tauri/src/lib.rs` (Desktop), handle the notification action by emitting a `navigate-to-session` event.

In `src/lib/stores/app.svelte.ts`, the listener handles robust payload parsing and automatic session reconnection:

```typescript
listen<any>("navigate-to-session", (event) => {
    const payload = event.payload;
    let session_name: string | null = null;
    let tab_index = 0;

    // Handle both stringified JSON (from Actions) and objects (from Intents)
    if (typeof payload === 'string') {
        try {
            const parsed = JSON.parse(payload);
            session_name = parsed.session_name;
            tab_index = parsed.tab_index ?? 0;
        } catch {
            session_name = payload;
        }
    } else if (payload && typeof payload === 'object') {
        session_name = payload.session_name;
        tab_index = payload.tab_index ?? 0;
    }

    if (session_name) {
        const session = sessions.find(s => s.zellijSession === session_name);
        if (session) {
            // Auto-connect if disconnected
            if (session.status !== 'connected') {
                appState.connectSession(session.id);
            } else {
                appState.activeSessionId = session.id;
                if (tab_index > 0) {
                    // Use a slight timeout to ensure PTY is ready
                    setTimeout(() => {
                        const command = `zellij -s ${session.zellijSession} action go-to-tab ${tab_index}`;
                        invoke("run_remote_command", { config: buildSshConfig(session), command });
                    }, 500);
                }
            }
            appState.scrollToPane(0); // Ensure terminal pane is visible
        }
    }
});
```

### 4.3 KDE and OS Fallback

Note: The current version of `tauri-plugin-notification` (v2.x) does not yet support custom action buttons or Rust-side notification event listeners. 

- **Focus:** On Linux (KDE), clicking the notification itself will typically focus the application window by default.
- **Android:** Deep-linking when the app is in the background is handled via the `intent` plugin's `handle_notification_action` command, which emits the `navigate-to-session` event.
- **Foreground:** When the app is focused, the `agent-notification` event is emitted directly via the WebSocket loop in `daemon.rs`, triggering the in-app `AgentNotificationToast` which provides rich navigation buttons.

---

## 5. AI Agent Hooks

### 5.1 Architecture

Both Claude Code and Gemini CLI support shell-command hooks that fire at defined
lifecycle events.  The hooks run in the same terminal environment as the agent —
which means Zellij env vars are available automatically.

```
Agent blocked / finished turn
  └─ Hook fires (shell command)
       └─ zn notify "Claude needs input" "$QUESTION" --source claude_code
            └─ POST /api/v1/trigger/notify  (localhost)
                 └─ WS broadcast → Tauri → system notification
                      └─ Tap → navigate to Zellij pane
```

### 5.2 Claude Code hook

Claude Code hooks are configured in `~/.claude/settings.json`.  The relevant event
types:

- **`Notification`** — fires when Claude Code rings a bell / wants user attention.
  stdin JSON: `{"title": "...", "message": "..."}`.  This is the primary hook.
- **`Stop`** — fires when the agent finishes its turn and is waiting for input.
  stdin JSON: `{"session_id": "...", "stop_hook_active": false}`.  Useful for
  "agent is idle, check your phone" nudges.

Create `~/.local/bin/zelland-agent-notify` (chmod +x):

```bash
#!/usr/bin/env bash
# zelland-agent-notify — called by Claude Code / Gemini CLI hooks
# Reads JSON from stdin, extracts title+message, sends via zn notify.

set -euo pipefail

SOURCE="${1:-claude_code}"     # first arg = source tag

# Parse stdin JSON (requires jq)
INPUT=$(cat)
TITLE=$(echo "$INPUT" | jq -r '.title // "Agent notification"')
BODY=$(echo "$INPUT"  | jq -r '.message // .body // ""')

# If the agent included an AskUserQuestion or similar, surface it as question
QUESTION=$(echo "$INPUT" | jq -r '.question // ""')

zn notify "$TITLE" "$BODY" \
  --source "$SOURCE" \
  ${QUESTION:+--question "$QUESTION"}
```

`~/.claude/settings.json`:

```json
{
  "hooks": {
    "Notification": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "zelland-agent-notify claude_code"
          }
        ]
      }
    ],
    "Stop": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "zelland-agent-notify claude_code"
          }
        ]
      }
    ]
  }
}
```

The `Notification` hook is the primary one: it fires when Claude Code explicitly
wants to alert the user (long task done, blocking question, etc.) and carries the
message text in stdin.  The `Stop` hook is a fallback nudge that fires on every
turn boundary, even when there is nothing special to say — consider only enabling it
for long-running tasks.

### 5.3 Gemini CLI hook

Gemini CLI (as of v1.x) supports hooks via `~/.gemini/settings.json` with an
analogous structure.  The event name may differ; check `gemini help hooks` for the
current list.  The same `zelland-agent-notify` script works unchanged:

```json
{
  "hooks": {
    "notification": [
      {
        "command": "zelland-agent-notify gemini_cli"
      }
    ]
  }
}
```

The stdin JSON format from Gemini CLI is expected to have `title` and `message`
fields matching the script.  Verify against the CLI's hook documentation if the
field names differ.

### 5.4 Including the blocking question

When Claude Code asks a clarifying question using `AskUserQuestion`, the question
text appears in the conversation but is not automatically available to hooks.  Two
approaches:

**Option A — Post-process the transcript (Stop hook):**
The `Stop` hook receives `transcript_path` in its stdin.  Parse the last assistant
message and extract question text:

```bash
# In zelland-agent-notify when SOURCE=claude_code and INPUT has transcript_path
TRANSCRIPT=$(echo "$INPUT" | jq -r '.transcript_path // ""')
if [ -n "$TRANSCRIPT" ]; then
  LAST_MSG=$(jq -r '.messages[-1].content[-1].text // ""' "$TRANSCRIPT" 2>/dev/null || true)
  QUESTION="$LAST_MSG"
fi
```

**Option B — AskUserQuestion tool output (preferred):**
Because `AskUserQuestion` is a tool call, its invocation fires the `PostToolUse`
hook with the full tool input including the question text.  Hook on
`AskUserQuestion`:

```json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "AskUserQuestion",
        "hooks": [
          {
            "type": "command",
            "command": "zelland-ask-notify"
          }
        ]
      }
    ]
  }
}
```

`~/.local/bin/zelland-ask-notify`:

```bash
#!/usr/bin/env bash
# Fires when Claude Code calls AskUserQuestion
# stdin JSON: {"tool_name": "AskUserQuestion", "tool_input": {"questions": [...]}, ...}

INPUT=$(cat)
FIRST_Q=$(echo "$INPUT" | jq -r '.tool_input.questions[0].question // "Claude needs input"')

zn notify "Claude needs your answer" "$FIRST_Q" \
  --source claude_code \
  --question "$FIRST_Q"
```

This is the cleanest path: the notification carries the exact question text, the
user can read it on the phone and decide whether to walk over to the machine.

---

## 6. Platform Matrix

| Feature | Android | KDE Linux |
|---|---|---|
| System notification | `tauri-plugin-notification` → Android NotificationManager | `tauri-plugin-notification` → libnotify → plasma-workspace |
| In-app banner | `navigate-to-session` Tauri event | Same event |
| Tap action | Intent → `handle_notification_action` → `navigate-to-session` | Notification click + `set_focus()` |
| Action button | Not required (tap is sufficient) | "Focus Terminal" button |
| Session switch | `setActiveSession` + `ssh_write zellij action go-to-tab N` | Same |
| Transport | WireGuard tunnel → daemon WS | Localhost daemon WS |

---

## 7. Security

- The `/api/v1/trigger/notify` endpoint is inside the `trigger_routes` block which
  has `loopback_guard` middleware.  Only processes on localhost can fire it.
- On Android, the WireGuard interface is the only path to the daemon.  The daemon
  binds to all interfaces so it can accept WS connections from zelland across the
  tunnel, but the trigger endpoint is still loopback-only.
- The notification payload travels over the WireGuard-encrypted tunnel.  No
  plaintext notification content is exposed to the network.
- Agent hooks run as the user's own shell process and post to localhost.  There is
  no privilege escalation.

---

## 8. Implementation Order

1. **Proto** — add `NavigationTarget`, `NotificationSource`, extend `Notification`.
   Run `cargo build` in both crates to regenerate bindings.
2. **Daemon handler** — `trigger_notify` + register route.  Manual test with `curl`.
3. **`zn notify` subcommand** — add to `cli.rs`.  Test from inside a Zellij pane.
4. **Tauri client** — attach action payload to notification; listen for tap intent.
5. **Frontend** — `navigate-to-session` listener in `app.svelte.ts`.
6. **Agent hooks** — install `zelland-agent-notify` + `zelland-ask-notify` scripts;
   configure `~/.claude/settings.json` and `~/.gemini/settings.json`.
7. **KDE "Focus" button** — add Linux-conditional action; handle in Tauri event loop.
