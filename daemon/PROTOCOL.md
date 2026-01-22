# Zelland Protocol Specification

This document defines the communication protocol between the **Zelland Daemon** (`zellandd`) running on the host and the **Zelland Android App**. It also covers the internal IPC used by the `zelland` CLI.

## 1. Overview

*   **Default Port**: `8083`
*   **Transport**: WebSocket (Secure `wss://` recommended over Tailscale, `ws://` for dev).
*   **Data Format**: Protocol Buffers (Protobuf).
*   **Endpoint**: `/ws`

## 2. WebSocket Communication

All messages are wrapped in an `Envelope` message.

### 2.1 Handshake & KeepAlive
Upon connection, the server sends an immediate `Ping`. The client should respond or simply acknowledge.

*   **Server -> Client**: `Envelope.Ping`
    ```protobuf
    message KeepAlive {
        int64 timestamp = 1;
    }
    ```
*   **Client Behavior**: Log the ping; optionally reply with a `ClientStatus` (future use).

### 2.2 Opening a View (Server -> Client)
Triggered when the user runs `zelland show <file>` or `zelland md <file>` on the host.

*   **Message**: `Envelope.OpenView`
    ```protobuf
    message OpenViewRequest {
        string asset_id = 1;    // Unique ID for the session
        string url = 2;         // Full or relative URL (e.g., "/assets/x9fk2m")
        FileType file_type = 3; // IMAGE (1) or MARKDOWN (2)
        string title = 4;       // Filename or custom title
    }
    ```

*   **Client Behavior**:
    1.  **Parse** the message.
    2.  **Construct** the full URL: `https://<host>:8083<url>`.
    3.  **UI Action**:
        *   Open a **new tab/window** distinct from the main Terminal session.
        *   **If IMAGE**: Display in a zoomable Image Viewer (or WebView).
        *   **If MARKDOWN**: Render the Markdown content. It is recommended to fetch the content from the `url` and render it natively or use a specialized WebView with text selection capabilities.
    4.  **User Experience**: The user should be able to close this tab to return to the terminal.

### 2.3 Annotations (Bidirectional)
Used for syncing highlights and notes on Markdown files.

*   **Message**: `Envelope.Annotation`
    ```protobuf
    message AnnotationAction {
        ActionType type = 1;    // CREATE (0), UPDATE (1), DELETE (2)
        string file_path = 2;   // The 'asset_id' from OpenViewRequest
        AnnotationData data = 3;
    }

    message AnnotationData {
        string id = 1;          // UUID
        string target_text = 2; // Selected text
        string context_hash = 3;// SHA256 of the surrounding paragraph
        string body = 4;        // User's comment
        int64 timestamp = 5;
    }
    ```

*   **Client Behavior (Sending)**:
    1.  User selects text in the Markdown view.
    2.  User taps "Annotate" / "Add Note".
    3.  App generates a UUID and captures the selection.
    4.  App sends `AnnotationAction (CREATE)` to the WebSocket.

*   **Server Behavior**:
    1.  Receives the action.
    2.  Writes the note to `<filename>.kdl` sidecar on the host.
    3.  (Optional) Broadcasts back to other clients.

## 3. IPC (CLI -> Daemon)

The CLI communicates with the daemon via local HTTP POST requests.

### 3.1 Trigger Show
*   **Endpoint**: `POST http://localhost:8083/api/v1/trigger/show`
*   **Body**:
    ```json
    {
        "file_path": "/absolute/path/to/image.png",
        "title": "image.png"
    }
    ```

### 3.2 Trigger Markdown
*   **Endpoint**: `POST http://localhost:8083/api/v1/trigger/md`
*   **Body**:
    ```json
    {
        "file_path": "/absolute/path/to/notes.md",
        "title": "notes.md"
    }
    ```

## 4. Asset Access
*   **Endpoint**: `GET http://localhost:8083/assets/{asset_id}`
*   **Behavior**: Serves the raw file content.
