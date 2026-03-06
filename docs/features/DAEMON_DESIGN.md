# zelland Companion Daemon Design

## 1. Architectural Overview

The **zelland Daemon** (`zlnd`) is a backend service written in **Rust**, running on the remote host (Linux). It acts as a bridge between the host's filesystem and the zelland application (Android + Desktop), enabling rich media interactions, collaborative annotations via YJS CRDT, and workflow enhancements beyond the terminal emulator.

> **Migration note (2026-02):** The daemon was originally written in Go (~1,067 lines). It has been rewritten in Rust to enable first-class integration with the `yrs` crate (Rust port of YJS) for real-time collaborative annotation sync. The REST API, WebSocket protocol, and protobuf message format remain backward-compatible.

### High-Level Components

1. **Daemon (`zlnd`)**: A persistent background service managing:
   * **HTTP/WebSocket Server** (`axum`): REST API + live control channel with the app.
   * **Asset Server**: HTTPS server serving content via obfuscated, ephemeral URLs.
   * **Annotation Engine** (`yrs`): YJS CRDT document hosting, sync, and persistence to `.ann.kdl` sidecar files.
   * **File Watcher** (`notify`): Monitors served files for changes, broadcasts updates.
2. **CLI (`zn`)**: A lightweight command-line tool that communicates with the daemon via local HTTP to trigger actions (e.g., `zn show image.png`).
3. **Client**: The zelland app (Tauri + Svelte), connecting via HTTP REST and WebSocket.

## 2. Communication Protocols

### 2.1 Control Channel (WebSocket)

* **Transport**: Secure WebSocket (`wss://<host>:<port>/ws`).
* **Format**: **Protocol Buffers (Protobuf)** via the `prost` crate.
* **Purpose**: Instant command delivery (Server -> Client), interaction events (Client -> Server), and system notifications.

### 2.2 Asset Transfer (HTTPS)

* **Transport**: HTTPS (served by the same `axum` server).
* **Security**: Capability URLs + Loopback protection for trigger endpoints.
* **Access Control**: **Capability URLs**. The daemon generates random, unguessable IDs for assets (e.g., `/assets/7f8a9d2b-4c1e`) mapped to specific file resources.

### 2.3 Wake-Up Mechanism (FCM) - *Future*

* **Service**: Firebase Cloud Messaging.
* **Scenario**: If the WebSocket connection is dead (app backgrounded/killed) when the Daemon needs to deliver an urgent message to the app.

## 3. Core Features & Workflows

### 3.1 The `show` Command (Universal Viewer)

**Goal**: Display an image, PDF, or generic file on the phone immediately.

**Workflow**:

1. **User**: Runs `zn show ./diagram.png` (CLI tool) on the remote host.
2. **CLI**: Sends a POST request to `zlnd`'s `/api/v1/trigger/show` endpoint (Loopback only).
3. **Daemon**:
   * Generates a random asset ID.
   * Maps `/assets/{id}` -> `./diagram.png`.
   * Broadcasts an `OpenViewRequest` Protobuf message via WebSocket to all connected clients.
4. **Client**:
   * Receives `OpenViewRequest`.
   * Opens a viewer pane.
   * Fetches content from `https://<host>:<port>/assets/{id}`.

### 3.2 The `md` Command (Markdown & Annotations)

**Goal**: Read Markdown files and enable real-time collaborative annotations.

**Workflow**:

1. **User**: Runs `zn md ./notes.md`.
2. **CLI** -> **Daemon**: Request to trigger Markdown view.
3. **Daemon**:
   * Registers `./notes.md` as an asset.
   * Broadcasts `OpenViewRequest` with `file_type = MARKDOWN`.
4. **Client**:
   * Renders Markdown in `MarkdownPane.svelte`.
   * Establishes a YJS sync connection to `ws://<host>:<port>/annotations/sync/path/to/notes.md`.
5. **Collaboration**:
   * Daemon manages a `yrs::Doc` for the annotation sidecar (`.ann.kdl`).
   * Real-time edits are synced via the YJS WebSocket protocol.

## 4. Data Structures (Protobuf Definitions)

The shared `proto/zelland.proto` defines the WebSocket envelope:

```protobuf
message Envelope {
  oneof payload {
    KeepAlive ping = 1;
    OpenViewRequest open_view = 2;
    AnnotationAction annotation = 3;
    ClientStatus status = 4;
    Notification notification = 5;
    ListSessionsRequest list_sessions_req = 6;
    ListSessionsResponse list_sessions_res = 7;
    CreateSessionRequest create_session_req = 8;
  }
}
```

## 5. Storage Strategy (KDL Sidecars)

Annotations are stored in human-readable KDL files alongside the source Markdown, using the `.ann.kdl` extension.

**File**: `project/README.md`
**Sidecar**: `project/README.ann.kdl`

**KDL Format Example**:

```kdl
ann "k8f2a" {
    selector {
        quote "ESP32-S3"
        prefix "architecture of the "
        suffix " microcontroller"
    }
    thread {
        comment id="c001" author="njr" created="2026-02-10T14:30:00Z" {
            body "Should we verify the power draw in deep sleep mode?"
        }
    }
}
```

The daemon provides a bridge between this KDL format and the in-memory YJS document state.

## 6. Security Considerations

1. **Loopback Guard**: Trigger endpoints (`/api/v1/trigger/*`) are restricted to loopback connections (127.0.0.1) to ensure only the local `zelland` CLI can initiate broadcasts.
2. **Path Security**: `daemon_read_file` and asset serving check that requested paths are within allowed project roots.
3. **Capability URLs**: Randomized asset paths prevent unauthorized document discovery.

## 7. Tech Stack (Rust)

| Concern | Crate | Notes |
|---------|-------|-------|
| HTTP/WS server | `axum` + `tokio` | Modern async web framework |
| WebSocket | `axum` + `tokio-tungstenite` | Protobuf + YJS binary sync |
| Protobuf | `prost` | High-performance protobuf generated code |
| CRDT/YJS | `yrs` | Rust implementation of YJS |
| File Watching | `notify` | Filesystem event monitoring |
| CLI Args | `clap` | Type-safe argument parsing |
| KDL | `kdl` | Sidecar persistence |

## 8. Implementation Roadmap

The Rust daemon implementation is complete, including:
- [x] Full REST API parity with the legacy Go daemon.
- [x] YJS CRDT integration for annotations.
- [x] Binary protobuf WebSocket messaging.
- [x] Asset management with TTL-based expiry.
- [x] Local `zn` CLI trigger tool.
