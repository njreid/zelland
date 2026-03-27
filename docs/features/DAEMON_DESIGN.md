# zelland Companion Daemon Design

> **Status note (2026-03):** This document describes the current daemon API and behavior that must be preserved. The active migration plan for turning `zlnd` into a per-user helper installed in `~/bin` lives in `docs/features/PER_USER_HELPER_DESIGN.md`.

## 1. Architectural Overview

The **zelland Daemon** (`zlnd`) is a backend service written in **Rust**, running on the remote host (Linux). It acts as a bridge between the host's filesystem and the zelland application (Android + Desktop), enabling rich media interactions, collaborative annotations via YJS CRDT, and workflow enhancements beyond the terminal emulator.

> **Migration note (2026-02):** The daemon was originally written in Go (~1,067 lines). It has been rewritten in Rust to enable first-class integration with the `yrs` crate (Rust port of YJS) for real-time collaborative annotation sync. The REST API, WebSocket protocol, and protobuf message format remain backward-compatible.

### High-Level Components

1. **Daemon (`zlnd`)**: A persistent background service managing:
   * **HTTP/WebSocket Server** (`axum`): REST API + live control channel with the app.
   * **Asset Server**: HTTPS server serving content via obfuscated, ephemeral URLs.
   * **Annotation Engine** (`yrs`): YJS CRDT document hosting, sync, and persistence to `.ann.kdl` sidecar files.
   * **File Watcher** (`notify`): Monitors served files and project directories for changes. Automatically reloads Loro CRDT state from Markdown if external changes are detected in the `# Comments` section.
2. **CLI (`zn`)**: A lightweight command-line tool that communicates with the daemon via local HTTP to trigger actions (e.g., `zn show image.png`).
3. **Client**: The zelland app (Tauri + Svelte), connecting via HTTP REST and WebSocket.

## 2. Core Features & Workflows

### 3.1 Project Synchronization & Watching

The daemon automatically monitors Markdown files within "active" projects:

1. **Activation**: When a client activates a project (via `/api/v1/projects/activate`), the daemon starts a recursive file watcher on the project root.
2. **Live Preview**: Any modification to a `.md` file within a watched project triggers an `OpenViewRequest` broadcast, causing connected clients to reload the live preview.
3. **Annotation Sync**: If the `# Comments` section of a Markdown file is edited externally (e.g., via a standard text editor), the file watcher triggers a Loro CRDT reload. The new comments are merged into the in-memory state and broadcast to all clients, ensuring the annotation sidebar stays in sync with the on-disk state.


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
