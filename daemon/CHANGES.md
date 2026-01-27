# Zelland Daemon - Development Milestones

This document tracks the incremental development of the `zellandd` companion daemon and the `zelland` CLI.

## Milestone 1: Foundations & Protocol
**Goal**: Establish the Go project, define the communication protocol (Protobuf), and create a basic WebSocket echo server.

- [x] **Project Initialization**
    - [x] Create `daemon/` directory structure (`cmd`, `internal`, `pkg`, `proto`).
    - [x] Initialize `go.mod` (e.g., `github.com/zelland/daemon`).
- [x] **Protocol Definition**
    - [x] Create `proto/zelland.proto` based on `DAEMON_DESIGN.md`.
    - [x] Define messages: `Envelope`, `KeepAlive`, `OpenViewRequest`, `AnnotationAction`.
    - [x] Set up `protoc` generation script (Makefile or Taskfile).
    - [x] Generate Go code from proto.
- [x] **Daemon Core (`zellandd`)**
    - [x] Implement `main` entry point with flag parsing (`-port`, `-config`).
    - [x] Implement HTTP server with WebSocket upgrade handler (using `gorilla/websocket` or `nhooyr.io/websocket`).
    - [x] Implement basic connection manager (handling client connect/disconnect).
    - [x] **Test**: Verify `wscat` can connect to the daemon and exchange ping/pong.

## Milestone 2: The `show` Command & Asset Pipeline
**Goal**: Allow the CLI to trigger an image display on a connected client.

- [x] **Asset Server**
    - [x] Implement an in-memory map of `AssetID -> LocalFilePath`.
    - [x] Implement HTTP handler `GET /assets/{asset_id}` that serves the file content.
    - [x] Add MIME type detection.
- [x] **CLI Tool (`zelland`)**
    - [x] Create `cmd/zelland` entry point.
    - [x] Implement `show <file>` subcommand.
    - [x] Implement IPC client (using HTTP RPC or UDS) to talk to the running `zellandd`.
        - *Design Decision*: Simple HTTP endpoint on localhost (`POST /api/v1/trigger/show`) is easiest for Phase 1.
- [x] **Daemon "Show" Logic**
    - [x] Implement `POST /trigger/show` handler.
    - [x] Logic: Validate file -> Generate Asset ID -> Store in Asset Map.
    - [x] Construct `OpenViewRequest` protobuf message.
    - [x] Broadcast message to active WebSocket connections.
- [x] **End-to-End Test (Manual)**
    - [x] Run `zellandd`.
    - [x] Connect a mock WebSocket client (or simple Go test script).
    - [x] Run `zelland show test_image.png`.
    - [x] Verify mock client receives `OpenViewRequest` with a valid URL.
    - [x] Verify `curl <url>` downloads the image.

## Milestone 3: Markdown & KDL Annotations
**Goal**: Support opening Markdown files and syncing annotations via KDL sidecars.

- [x] **KDL Handling**
    - [x] Import a Go KDL parser (e.g., `github.com/sblinch/kdl-go`).
    - [x] Implement `Store` interface for reading/writing `filename.kdl`.
    - [x] Define struct mapping for `Annotation` to KDL nodes.
- [x] **Markdown Session Logic**
    - [x] Implement `zelland md <file>` subcommand.
    - [x] Daemon handler:
        - [x] Read `.md` file content.
        - [x] Load existing annotations from `.kdl`.
        - [x] Send `OpenViewRequest` (type=MARKDOWN) + initial payload (content + notes).
- [x] **Annotation Sync (Server Side)**
    - [x] Handle incoming WebSocket `AnnotationAction` messages.
    - [x] Logic:
        - [x] Parse action (Create/Update/Delete).
        - [x] Update in-memory model.
        - [x] Flush changes to `.kdl` file on disk.
        - [x] Broadcast update to other clients (optional for v1).
- [x] **Test**: Unit tests for KDL serialization/deserialization.

## Milestone 4: Notifications & Hardening
**Goal**: Ensure reliability with FCM wake-ups and security improvements.

- [ ] **FCM Integration (Optional/Later)**
    - [ ] Add Firebase Admin SDK to daemon.
    - [ ] Implement logic: If no WebSocket active, send FCM data message "Wake Up".
- [x] **Security**
    - [x] Implement random path generation for assets (`/assets/7f8a...`).
    - [x] Implement auto-cleanup (expire assets after N minutes or session end).
    - [x] Bind IPC/RPC endpoints to loopback only.
    - [x] Implement TLS (Self-signed) for WebSocket and Asset Server.
- [x] **Configuration**
    - [x] Add config file support (YAML/TOML) for listening ports, storage paths, etc. (Implemented JSON for now)

## Milestone 5: Android Integration Support
**Goal**: Provide necessary endpoints or mocks to help Android development.

- [x] **Mock Client**: Create a small Go program that simulates the Android app (connects WS, prints messages, replies to pings) to facilitate daemon testing without the phone.
- [x] **Documentation**: Generate API docs or an OpenAPI spec for the local RPC interface.
