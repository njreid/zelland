# Annotation System Design (Loro + In-Doc Storage)

Collaborative annotations for Markdown documents stored directly within the document source using Loro CRDT for real-time synchronization.

## Overview

Users can annotate text within Markdown previews. Unlike the previous sidecar approach, annotations and comment threads are stored **directly in the Markdown file** in a hidden-by-default footer section. Real-time collaboration is handled via **Loro CRDT** sync between the Svelte client and the Rust daemon.

## Storage: In-Doc Markdown

Annotations are stored using two components: an **inline anchor link** and a **Comments section** at the end of the file.

### Inline Anchor Link

Annotated text is wrapped in a standard Markdown link pointing to a header ID in the footer:

```markdown
The architecture of the [ESP32-S3](#k8f2a) microcontroller supports dual-core processing.
```

- **Syntax:** `[quoted text](#ID)`
- **ID:** A unique 5-character alphanumeric ID (e.g., `k8f2a`).
- **Benefit:** Standard Markdown renderers treat this as a internal document link. External editors can move or edit the text without breaking the reference.

### Footer Section: `# Comments`

The end of every annotated Markdown file contains a `# Comments` H1 section. Each annotation ID has its own H2 sub-header, followed by a list of comments.

```markdown
... document content ends here ...

# Comments

## k8f2a

- 2026-02-10T14:30:00Z njr: Should we verify the power draw in deep sleep mode?
- 2026-02-10T15:12:00Z alice: Yes - measured at **45uA** in deep sleep.

## m3x9p

- 2026-02-11T09:00:00Z njr: This is done - see `network.rs`.
```

- **Header:** `## ID` matches the anchor in the text.
- **Comment Format:** `- TIMESTAMP author: body`
- **Body:** Supports inline Markdown (bold, links, etc.).

## Collaboration: Loro CRDT Sync

### Why Loro?

[Loro](https://loro.dev) is a high-performance CRDT framework built in Rust. It is selected for zelland due to:

- **Shallow Snapshots:** Efficiently handles large document histories by truncating state while preserving mergeability.
- **First-class Rust & JS support:** Seamless integration between the Tauri/Rust daemon and Svelte/JS frontend.
- **Fugue Algorithm:** Superior text interleaving prevention compared to Yjs's YATA.

### Architecture

```text
  Svelte Client A                    Svelte Client B
       |                                  |
       |  WebSocket (Loro binary sync)    |  WebSocket (Loro binary sync)
       v                                  v
  +--------------------------------------------------+
  |              zlnd (daemon)                       |
  |                                                   |
  |  Loro Doc (authoritative state)                   |
  |       |                                           |
  |       +---> Reify to Markdown (# Comments)        |
  +--------------------------------------------------+
```

### Loro Document Structure

The Loro document models the annotation state using a nested map structure:

```text
LoroDoc
  annotations: Map
    "k8f2a": List (Comments)
      [0]: Map { author: "njr", timestamp: "...", body: "..." }
      [1]: Map { author: "alice", timestamp: "...", body: "..." }
```

### Sync & Persistence

1. **Connection:** Client connects via `ws://<host>:8083/annotations/sync/<filepath>`.
2. **Cold Start:** Daemon reads the Markdown file, parses the `# Comments` section, and populates the Loro document.
3. **Delta Sync:** Loro handles incremental updates via binary blobs over WebSockets.
4. **Reification:** When the Loro document changes, the daemon updates the `# Comments` section in the Markdown file on disk.

## Frontend Experience

### Markdown Rendering

The `MarkdownPane` uses a custom `marked` extension to process the doc:

1. **Highlighting:** Intercepts `[text](#ID)` links. If the ID exists in the annotation state, it renders as an `<span class="ann-highlight">` with a blue underline and background.
2. **Interception:** Prevents the browser from jumping to the footer when an anchor is clicked. Instead, it scrolls the **Sidebar** to the relevant thread.
3. **Footer Hiding:** The `# Comments` section is **stripped from the visible preview**. It is strictly used as a data source for the sidebar.

### Annotation Sidebar (Desktop)

- Displays all threads ordered by their appearance in the document.
- **Add Annotation:** User selects text -> "Annotate" -> New ID generated -> Daemon inserts `[text](#ID)` and populates the footer.

### Mobile Experience

- No sidebar. Tapping an highlighted link expands a floating bottom sheet or inline card containing the `## ID` thread from the footer.

## Daemon Implementation (Rust)

- **Loro Integration:** Uses the `loro` crate to manage state.
- **Markdown Parser:** Uses a line-based parser or `pulldown-cmark` to surgically update the `# Comments` section without corrupting the rest of the document.
- **Conflict Resolution:** If two users edit the `# Comments` section manually while the daemon is offline, Loro merges the changes on the next start based on the binary history (stored in a small `.zelland/` hidden folder if needed, or derived from the Markdown text).

## Open Questions

1. **History Persistence:** Should we store the full Loro binary history in the Markdown file (e.g., in an HTML comment `<!-- loro:snapshot -->`)?
   - *Proposal:* Store only the reified text in Markdown. Store the Loro binary snapshot in a hidden local cache file on the host (`.<filename>.zelland`) in the same path to ensure fast restarts and robust merging.
2. **Manual Edits:** What happens if a user deletes a comment line in a standard text editor?
   - *Behavior:* The daemon will treat this as a deletion in the Loro doc during the next sync/load.
3. **Link Collisions:** How to distinguish "real" document links to headers from annotation anchors?
   - *Decision:* Reserve a specific ID pattern (e.g., exactly 5-8 alphanumeric chars)
