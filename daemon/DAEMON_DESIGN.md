# zelland Companion Daemon Design

## 1. Architectural Overview

The **zelland Daemon** (`zellandd`) is a backend service written in **Rust**, running on the remote host (Linux). It acts as a bridge between the host's filesystem and the zelland application (Android + Desktop), enabling rich media interactions, collaborative annotations via YJS CRDT, and workflow enhancements beyond the terminal emulator.

> **Migration note (2026-02):** The daemon was originally written in Go (~1,067 lines). It has been rewritten in Rust to enable first-class integration with the `yrs` crate (Rust port of YJS) for real-time collaborative annotation sync. The REST API, WebSocket protocol, and protobuf message format remain backward-compatible.

### High-Level Components

1. **Daemon (`zellandd`)**: A persistent background service managing:
   * **HTTP/WebSocket Server** (`axum`): REST API + live control channel with the app.
   * **Asset Server**: HTTPS server serving content via obfuscated, ephemeral URLs.
   * **Annotation Engine** (`yrs`): YJS CRDT document hosting, sync, and persistence to `.ann.kdl` sidecar files.
   * **File Watcher** (`notify`): Monitors served files for changes, broadcasts updates.
2. **CLI (`zelland`)**: A lightweight command-line tool that communicates with the daemon via local HTTP to trigger actions (e.g., `zelland show image.png`).
3. **Client**: The zelland app (Tauri + Svelte), connecting via HTTP REST and WebSocket.

## 2. Communication Protocols

### 2.1 Control Channel (WebSocket)

* **Transport**: Secure WebSocket (`wss://<host>:<port>/ws`).
* **Format**: **Protocol Buffers (Protobuf)**.
* **Purpose**: Instant command delivery (Server -> Client) and interaction events (Client -> Server). WebSocket messages will be tuned to minimize battery consumption, ping/pong frames, fast dormancy etc.

### 2.2 Asset Transfer (HTTPS)

* **Transport**: HTTPS.
* **Security**: Relies on Tailscale/Private Network for transport security.
* **Access Control**: **Capability URLs**. The daemon generates random, unguessable paths (e.g., `/view/7f8a9d2b-4c1e`) linked to specific file resources.

### 2.3 Wake-Up Mechanism (FCM)

* **Service**: Firebase Cloud Messaging.
* **Scenario**: If the WebSocket connection is dead (app backgrounded/killed) when the Daemon needs to deliver an urgent message to the app, the Daemon sends a high-priority data message via FCM to prompt the Android app to wake up and reconnect the WebSocket.

## 3. Core Features & Workflows

### 3.1 The `show` Command (Universal Viewer)

**Goal**: Display an image, PDF, or generic file on the phone immediately.

**Workflow**:

1. **User**: Runs `zel show ./diagram.png` in the terminal.
2. **CLI**: Validates file existence and sends a request to `zellandd` (Local RPC).
3. **Daemon**:
   * Generates a random ID: `x9fk2m`.
   * Maps `/assets/x9fk2m` -> `./diagram.png` in memory.
   * Constructs a `ViewRequest` Protobuf message.
   * Sends `ViewRequest` via WebSocket to the connected Android client.
4. **Android**:
   * Receives `ViewRequest`.
   * Opens a new "Viewer Tab" (separate from Terminal).
   * Loads `https://<host>:<port>/assets/x9fk2m` in a WebView.
   * User can zoom/pan/close.

### 3.2 The `md` Command (Markdown & Annotations)

**Goal**: Read Markdown files and save annotations back to the server.

**Workflow**:

1. **User**: Runs `zel ann ./notes.md`.
2. **CLI** -> **Daemon**: Request to serve `./notes.md`.
3. **Daemon**:
   * Reads `./notes.md`.
   * Checks for/Reads `./notes.kdl` (Sidecar Annotations).
   * Generates asset ID `md_72j1`.
   * Sends `MarkdownSessionStart` Protobuf to Android (contains URL + existing annotations).
4. **Android**:
   * Renders Markdown natively or via specialized WebView.
   * User selects text -> Taps "Annotate".
   * App sends `AnnotationCreate` Protobuf message to Daemon.
5. **Daemon**:
   * Receives `AnnotationCreate` (contains context hash, selected text, note body).
   * Updates/Creates `./notes.kdl`.
   * Broadcasts update to any other listeners (future proofing).

## 4. Data Structures (Protobuf Definitions)

*Draft `zelland.proto`*

```protobuf
syntax = "proto3";

package zelland;

// Wrapper for all WebSocket messages
message Envelope {
  oneof payload {
    KeepAlive ping = 1;
    OpenViewRequest open_view = 2;
    AnnotationAction annotation = 3;
    ClientStatus status = 4;
  }
}

message OpenViewRequest {
  string asset_id = 1;
  string url = 2; // https://host/assets/xyz
  enum FileType {
    UNKNOWN = 0;
    IMAGE = 1;
    MARKDOWN = 2;
    PDF = 3;
  }
  FileType file_type = 3;
  string title = 4;
  // For Markdown, initial state might be included here or fetched separately
}

message AnnotationAction {
  enum ActionType {
    CREATE = 0;
    UPDATE = 1;
    DELETE = 2;
  }
  ActionType type = 1;
  string file_path = 2; // Or asset_id
  AnnotationData data = 3;
}

message AnnotationData {
  string id = 1;
  string target_text = 2;
  string context_hash = 3; // SHA of surrounding paragraph for robust anchoring
  string body = 4; // The user's note
  int64 timestamp = 5;
}
```

## 5. Storage Strategy (KDL Sidecars)

Inspired by the `ORGD` design, annotations are stored in human-readable KDL files alongside the source.

**File**: `project/README.md`
**Sidecar**: `project/README.kdl`

**KDL Format Example**:

```kdl
annotation id="uuid-v4" timestamp=176982342 {
    target "installation instructions"
    context_hash "sha256:8f2..."
    body "This step fails on Ubuntu 24.04, need to update."
}
```

## 6. Security Considerations

1. **Network Scope**: The daemon binds to `0.0.0.0` but serves traffic primarily over Tailscale interfaces (recommended).
2. **Asset Obsolescence**:
   * "Show" assets (images) expire after X minutes of inactivity or when the tab is closed on Android (client sends `SessionClosed` event).
   * Randomized paths prevent directory traversal or guessing.
3. **Authentication**:
   * Simple Pre-Shared Key (PSK) or Token exchange on WebSocket handshake, configured via environment variable or config file on both Host and Phone.

## 7. Tech Stack (Rust)

| Concern | Crate | Notes |
|---------|-------|-------|
| HTTP/WS server | `axum` + `tokio` | Shared runtime with async I/O |
| WebSocket | `axum` (built-in via `tokio-tungstenite`) | Binary protobuf + YJS sync |
| Protobuf | `prost` + `prost-build` | Same `zelland.proto` as before |
| KDL config/storage | `kdl` | Parse/write `.kdl` and `.ann.kdl` files |
| File watching | `notify` | Cross-platform filesystem events |
| CRDT/YJS | `yrs` + `y-sync` | Collaborative annotation documents |
| CLI args | `clap` | `zellandd --config daemon.kdl --port 8083` |
| TLS | `axum-server` + `rustls` | Optional, for `cert_file`/`key_file` |
| Logging | `tracing` + `tracing-subscriber` | Structured logging |

## 8. Implementation Roadmap

See `PLAN.md` Phase 6 for detailed task breakdown. Summary:

1. **6A–6C**: Scaffold, config, data types, asset manager — pure library code with tests.
2. **6D–6F**: HTTP server, WebSocket, file watching — matches existing Go API 1:1.
3. **6G**: CLI binaries (`zellandd`, `zelland`).
4. **6H**: YJS integration — `yrs` doc hosting, sync endpoint, KDL↔YJS bridge.
5. **6I**: Integration testing against Tauri client, cutover from Go.
