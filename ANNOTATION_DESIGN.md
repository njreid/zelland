# Annotation System Design

Collaborative annotations for Markdown documents viewed in zelland's MarkdownPane.

## Overview

Users can annotate text within Markdown previews. Annotations are stored in **sidecar files** (`<docname>.ann.kdl`) using KDL format, with short anchor references embedded in the Markdown source. Real-time collaboration is handled via **YJS CRDT** sync between the Svelte client and the backend daemon.

## Storage

### Sidecar File: `<docname>.ann.kdl`

For `README.md`, the sidecar is `README.ann.kdl`, stored alongside the original file on the remote host.

```kdl
// README.ann.kdl

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
        comment id="c002" author="alice" created="2026-02-10T15:12:00Z" {
            body "Yes - measured at **45uA** in deep sleep. Added to the spec table."
        }
    }
}

ann "m3x9p" {
    selector {
        quote "userspace packet loop"
        prefix "Implement "
        suffix " in `src-tauri"
    }
    thread {
        comment id="c003" author="njr" created="2026-02-11T09:00:00Z" {
            body "This is done - see `network.rs`."
        }
    }
}
```

### Anchor Reference in Markdown

Each annotation has a unique **~5-character alphanumeric ID** (e.g., `k8f2a`). This ID is embedded in the Markdown source as an inline anchor marker using the syntax:

```markdown
The architecture of the [|k8f2a|]ESP32-S3 microcontroller supports dual-core processing.
```

The `[|ID|]` marker is a zero-width anchor point placed immediately before the annotated text. The `selector.quote` field in the KDL file identifies the extent of highlighted text following the anchor.

**Why embed in the Markdown?** Pure standoff markup (offset-based) breaks when the file is edited. Embedding a short, stable reference ID means the anchor survives arbitrary edits to surrounding text. The `selector` fields (quote, prefix, suffix) provide a fallback for fuzzy re-anchoring if the marker is accidentally deleted.

### Anchor Resolution Algorithm

When rendering a Markdown file with annotations:

1. **Primary:** Scan for `[|ID|]` markers in the source. The annotated range starts immediately after the marker and extends for `len(selector.quote)` characters.
2. **Fallback (fuzzy):** If the `[|ID|]` marker is missing (e.g., removed by an external editor), use the selector fields:
   - Find paragraphs matching `prefix + quote + suffix` via fuzzy string matching.
   - Re-insert the `[|ID|]` marker at the resolved position.
3. **Orphaned:** If neither primary nor fuzzy resolution succeeds, the annotation is marked as orphaned and displayed in a separate "Unresolved" section.

## Frontend Experience

### Rendering Annotated Text

Anchor text (the `selector.quote` range following a `[|ID|]` marker) is rendered with:

- A **blue underline** beneath the anchor text.
- A **faint blue background highlight** on the text itself.
- A subtle superscript annotation count badge if the thread has multiple comments.

The Markdown renderer (`marked`) is extended with a custom tokenizer/renderer that:

1. Strips `[|ID|]` markers from visible output.
2. Wraps the subsequent `quote` text in `<span class="ann-anchor" data-ann-id="ID">...</span>`.

### Desktop Layout

When annotations exist for the current document:

- A **closable right-hand sidebar** appears alongside the MarkdownPane, containing all comment threads ordered by their position in the document (top to bottom).
- Each thread shows the quoted anchor text as a header, followed by the comment chain.
- **Clicking an anchor** in the document fast-scrolls the sidebar to the corresponding thread.
- **Clicking a thread** in the sidebar scrolls the document to the anchor.

**Adding an annotation (desktop):**

1. User selects text in the rendered Markdown.
2. The sidebar scrolls to the insertion point (between neighboring annotations, ordered by document position).
3. A reply box appears, pre-filled with the selected text as context.
4. On submit, a new `ann` node is created in the KDL sidecar with a generated ID, and the `[|ID|]` marker is inserted into the Markdown source.

### Mobile Layout

On mobile, there is no sidebar. Annotations are handled **inline**:

- **Tapping an anchor** expands a comment view directly below the anchor text in the document flow.
- By default, only the **last comment** in the chain is visible, along with a compact reply box.
- The reply box expands when tapped. Replies can be submitted or canceled.
- A "Show earlier" link expands the full chain above the latest reply.
- A close button collapses the inline view.

### Comment Format

Each comment displays:

- **Author name** (small, muted).
- **Relative timestamp** (e.g., "2h ago", "3 days ago") as a subtle indicator.
- **Body** rendered as Markdown (supports inline formatting: bold, italic, code, links).

## Collaboration: YJS CRDT Sync

### Architecture

```text
  Svelte Client A                    Svelte Client B
       |                                  |
       |  WebSocket (YJS sync)            |  WebSocket (YJS sync)
       v                                  v
  +--------------------------------------------------+
  |              zellandd (daemon)                    |
  |                                                   |
  |  YJS Doc (authoritative state)                    |
  |       |                                           |
  |       +---> Persist to <docname>.ann.kdl          |
  +--------------------------------------------------+
```

- The **daemon** hosts a YJS document per annotation file.
- Svelte clients connect via **WebSocket** to the daemon's YJS sync endpoint.
- All annotation mutations (add, edit, delete comments; create/remove anchors) are YJS operations.
- The daemon **persists** the YJS document state to the `.ann.kdl` file on disk periodically and on disconnect.
- On reconnect or cold start, the daemon **loads** the `.ann.kdl` file and initializes the YJS document.

### YJS Document Structure

The YJS document models the annotation state as a `Y.Map` of annotation objects:

```text
Y.Doc
  annotations: Y.Map<string, Y.Map>    // keyed by annotation ID
    "k8f2a": Y.Map
      selector: Y.Map { quote, prefix, suffix }
      thread: Y.Array<Y.Map>           // ordered list of comments
        [0]: Y.Map { id, author, created, body }
        [1]: Y.Map { id, author, created, body }
```

### Sync Protocol

1. Client opens a MarkdownPane with annotations.
2. Client connects to `ws://<host>:8083/annotations/<docpath>`.
3. Daemon creates or loads the YJS doc for that file.
4. Standard YJS WebSocket sync protocol (`y-websocket`) handles state exchange and incremental updates.
5. Client receives updates and re-renders annotation UI reactively.

### Conflict Resolution

YJS handles conflicts automatically:

- **Concurrent comment additions:** Both appear in the thread (ordered by timestamp).
- **Concurrent edits to the same comment:** Last-write-wins at the field level (YJS Map semantics).
- **Concurrent anchor deletion + comment addition:** The new comment attaches to the (now orphaned) annotation, which appears in the "Unresolved" section.

## Daemon API

### New Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/annotations/{filepath}` | Read annotations for a file (KDL or JSON) |
| `PUT` | `/annotations/{filepath}` | Write/update annotations (used for non-YJS fallback) |
| `WS` | `/annotations/{filepath}/sync` | YJS WebSocket sync endpoint |

### KDL Parsing

The daemon is written in Rust and uses the `kdl` crate to read/write `.ann.kdl` files. The JS client never parses KDL directly — it works with the YJS document via the sync protocol. The daemon handles the KDL ↔ YJS bridge:

- **Cold start:** Read `.ann.kdl` → populate `yrs::Doc` with annotation data.
- **Persistence:** Serialize `yrs::Doc` annotation state → write `.ann.kdl` (debounced, 5s timer + on shutdown).

## Implementation Components

### Svelte Client

| Component | Purpose |
|-----------|---------|
| `AnnotationSidebar.svelte` | Desktop: right-hand panel with threaded comments |
| `InlineAnnotation.svelte` | Mobile: inline expandable comment view |
| `AnnotationAnchor.svelte` | Rendered anchor span (blue underline + highlight) |
| `CommentThread.svelte` | Shared thread UI (list of comments + reply box) |
| `Comment.svelte` | Single comment (author, timestamp, markdown body) |
| `lib/annotations.ts` | YJS doc setup, WebSocket provider, reactive state |
| Custom `marked` extension | Tokenizer for `[|ID|]` markers, renderer for anchor spans |

### Daemon (Rust)

| Component | Purpose |
|-----------|---------|
| `store.rs` | KDL serialize/deserialize for `.ann.kdl` files |
| `yjs.rs` | `yrs::Doc` manager per file, KDL↔YJS bridge |
| `server.rs` | `/annotations/{filepath}/sync` WebSocket endpoint (`y-sync` protocol) |
| `anchor.rs` | Fuzzy re-anchoring: re-insert `[|ID|]` markers via prefix/suffix matching |

## Open Questions

1. **Anchor syntax:** Is `[|ID|]` the right marker syntax? Alternatives:
   `<!-- ann:ID -->` (HTML comment — invisible in all renderers, but verbose)
   `[^ID]` (footnote-like — conflicts with actual Markdown footnotes)
   `{#ID}` (attribute-like — might conflict with some Markdown extensions)
   `[|ID|]` is visually distinct and unlikely to collide, but is visible as literal text in renderers that don't understand it.

2. **Author identity:** How are comment authors identified? Options:
   Username from the SSH session
   Configurable display name in zelland settings
   Anonymous (no author tracking)

3. **Markdown source mutation:** Adding `[|ID|]` markers modifies the Markdown file. Should this:
   Happen immediately when the annotation is created?
   Be batched/deferred?
   Require a separate "save annotations" action?
   How does this interact with version control (git)? Should markers be committed?

4. **Permissions:** Can any connected user:
   Add annotations to any file?
   Edit/delete others' comments?
   Delete entire annotation threads?

5. ~~**KDL library for Go**~~ **Resolved:** Daemon migrated to Rust. Uses the `kdl` crate (well-maintained, idiomatic Rust).

6. **YJS persistence strategy:** How often should the daemon flush YJS state to disk?
   On every mutation (safe but slow)?
   On a debounced timer (e.g., every 5 seconds)?
   On client disconnect?
   On daemon shutdown?

7. **Offline / disconnected editing:** If a client is offline:
   Should annotations be cached locally and synced on reconnect?
   Or are annotations only available when connected to the daemon?

8. **File watching:** Should the daemon watch `.ann.kdl` files for external edits (e.g., from another tool)?

9. **Annotation scope:** Are annotations per-project or per-file? Can an annotation reference text across multiple files?

10. **Mobile text selection:** On mobile (Android WebView), selecting text in a rendered Markdown view to create an annotation may be difficult. What's the interaction for creating annotations on mobile?
    Long-press to select, then a floating "Annotate" button?
    A dedicated "annotation mode" toggle?
