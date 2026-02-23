# Zaplet System Design (Offline-First Applets via Loro + KDL)

A generic framework for small, purpose-built collaborative editors ("zaplets") that store their state as versioned KDL files, use Loro CRDT for real-time sync, and are served as single-page HTML/JS apps by the `zlnd` daemon.

---

## Motivation

The annotation system (see `ANNOTATION_DESIGN.md`) solves one specific problem: threading comments onto Markdown text. Zaplets generalize this pattern to arbitrary structured data embedded in a project:

- A task list stored alongside code (`work-tasks.todo.kdl`)
- A mindmap for a planning session (`sprint.mmap.kdl`)
- Image annotation markup (`ui-mockup.image.kdl`)
- A decision log (`adr-log.decisions.kdl`)

Each zaplet is:

- **Self-contained** — the KDL file is the single source of truth; it is human-readable, diffable, and parseable by standard KDL tools.
- **Offline-first** — the Loro CRDT doc in the daemon is the live state; the KDL is its reification to disk. Clients cache zaplet editors locally and continue editing without a daemon connection.
- **Schema-versioned** — the KDL file embeds its schema version; the daemon runs migrations on open.
- **Routable** — `zlnd` dispatches to the correct editor based on the file subtype (the segment between the last two dots, e.g. `.todo` in `work-tasks.todo.kdl`).

---

## File Naming Convention

```text
<name>.<subtype>.kdl
```

| File | Subtype | Zaplet |
|------|---------|--------|
| `work-tasks.todo.kdl` | `todo` | Task list |
| `sprint.mmap.kdl` | `mmap` | Mindmap canvas |
| `ui-mockup.image.kdl` | `image` | Image feature markup |
| `decision-log.decisions.kdl` | `decisions` | ADR / decision log |
| `retro.retro.kdl` | `retro` | Team retrospective board |

The double-extension pattern means:

- Ordinary text editors still open the file as KDL (valid text).
- `zlnd` can route without inspecting file contents (just the name).
- Git treats them as ordinary files.

---

## KDL File Structure

Every `.*.kdl` file begins with a mandatory `zaplet` header node followed by the type-specific payload. All KDL files conform to **KDL v2** syntax.

### General Shape

```kdl
zaplet todo version=3 created="2026-02-22T10:00:00Z" author=njr

// --- payload (type-specific) ---

task id=k8f2a {
    title "Fix the login bug"
    status in-progress
    due "2026-03-15"
    assignee njr
    tags backend auth
    created "2026-02-20T09:00:00Z"
}

task id=m3x9p {
    title "Write auth tests"
    status todo
    due #null
    assignee alice
    tags testing
    created "2026-02-21T14:30:00Z"
}

// Explicit ordering list (separate from map identity)
order k8f2a m3x9p
```

### Header Node

| Field | Type | Purpose |
|-------|------|---------|
| `version` | integer | Schema version; triggers migration chain on open |
| `created` | ISO 8601 string | File creation time |
| `author` | string | Creator identity |

The header is always the first node; the daemon validates it before loading the Loro doc.

### Canonical Format

Reification always produces a **canonical KDL v2 representation**:

- Two-space indentation throughout.
- Node children always follow a stable ordering defined by the schema (not insertion order).
- `#null` is emitted for explicitly null values; absent nodes represent fields that were never set. These are semantically distinct in the schema.
- KDL comments are not preserved — they are stripped on every reification. If information matters it belongs in the schema.
- Strings that contain spaces, `#`, `/`, or start with a digit are quoted; all others are bare identifiers.

---

## Loro Document Structure

The Loro doc is the authoritative in-memory state. The KDL file is derived from it. The mapping below defines how each Loro type reifies to KDL.

### Type Mapping

| Loro Type | KDL Representation | Use Case |
|-----------|-------------------|----------|
| `LoroMap` | Named node with properties and/or child nodes | Records, objects, indexed collections |
| `LoroList` | Named node with positional string or node children | Ordered sequences, tag lists, ordering indices |
| `LoroTree` / `LoroTreeNode` | Recursively nested named nodes | Hierarchical data: mindmaps, outlines, folder trees |
| `LoroText` | KDL string argument | Fields with type `(markdown)` in the schema only |
| Primitive (string/int/bool/float) | KDL argument or property | Leaf values |

**`LoroText` is reserved for schema fields typed `(markdown)`.** All other string fields use plain Rust strings with last-write-wins semantics. This distinction is encoded in the schema and must not be changed after a zaplet is deployed (it requires a schema migration and a Loro doc rebuild).

### Ordering

`LoroMap` does not guarantee insertion order. If a zaplet needs its items displayed in a user-controlled sequence, it must define either:
- An explicit `LoroList` alongside the map (the `order` node pattern in the todo example), or
- A `rank` field (float or integer) on each map item.

The schema declares which approach is used; the reifier respects it.

### Hierarchical Data and Cross-Links

`LoroTree` is used for strict parent-child hierarchies (mindmaps, outlines). Each node has exactly one parent — `LoroTree` natively prevents duplicate-parent anomalies during concurrent reparenting.

For DAG-like relationships (e.g. a node that logically appears in multiple branches), the primary parent is stored in the `LoroTree`; secondary references are stored as a `LoroList<string>` of IDs on the node's `LoroMap`. Secondary references are hints only and are not traversed by the reifier's tree walk.

### Example: Todo Zaplet

```text
LoroDoc
  tasks: LoroMap              // keyed by short random ID
    "k8f2a": LoroMap
      title: LoroText         // (markdown) typed — LoroText
      status: string          // plain LWW string; enum todo|in-progress|done
      due: string | null
      assignee: string | null
      tags: LoroList<string>
      created: string
  order: LoroList<string>     // explicit position list over task IDs
```

### Example: Mindmap Zaplet

```text
LoroDoc
  canvas: LoroMap
    offset_x: float
    offset_y: float
    zoom: float
  tree: LoroTree              // authoritative hierarchy
    root (LoroTreeNode)
      label: LoroText         // (markdown) typed
      color: string
      collapsed: bool
      x: float                // layout hint; non-authoritative
      y: float
      xrefs: LoroList<string> // secondary cross-links (IDs)
```

`LoroTree` handles move-without-duplicate semantics; simultaneous reparenting of the same node converges correctly.

### Example: Image Markup Zaplet

```text
LoroDoc
  image: LoroMap
    src: string               // relative URI; daemon serves the file
    width: integer
    height: integer
  annotations: LoroMap        // keyed by ID
    "a1b2c": LoroMap
      shape: string           // rect | ellipse | arrow | freehand
      x: float
      y: float
      w: float
      h: float
      label: LoroText         // (markdown) typed
      author: string
      color: string
      created: string
  order: LoroList<string>     // z-order
```

Binary image data is **never** stored in the Loro doc. The `src` field is a relative URI resolved against the project root; the daemon serves the file via its existing asset endpoint.

---

## Sidecar File (`.zel`)

Each zaplet KDL file has a corresponding hidden sidecar that stores the Loro binary history:

```text
work-tasks.todo.kdl   ← human-readable, committed to git
.work-tasks.todo.zel  ← Loro binary snapshot, gitignored
```

### Sidecar rules

- **Content:** A 30-day rolling Loro operation log (shallow snapshot). Older operations are pruned on every write.
- **Updated:** After every successful reification, subject to the 500 ms debounce (see Reification).
- **Gitignored:** Add `*.zel` to the project `.gitignore`. Sidecar files are not committed; teams are responsible for backing them up through other means (daemon-level backup, filesystem snapshots, etc.). Clones start with cold-load semantics.
- **On daemon start:** Load sidecar → merge with current KDL. If sidecar is absent, cold-load from KDL (no CRDT merge semantics for the gap).
- **External KDL edits (daemon running):** fswatch detects the write. The daemon performs a three-way diff: base KDL (last reified state, recoverable from sidecar metadata) + current Loro state + new KDL. The merged result is applied as a Loro update and reified back. (See Open Questions §1 for the diff algorithm.)
- **External KDL edits (daemon offline):** On next start, the daemon detects drift by comparing the KDL file's mtime against the sidecar's recorded last-reify timestamp. If drifted and no sidecar exists, cold-load. If sidecar exists but predates the KDL edit, treat the KDL as an external Loro update and merge.
- **Before migration:** Copy the current sidecar to `.work-tasks.todo.zel.bak.<n>` (retain last 3). If the migration fails, restore the most recent `.bak`.

---

## Reification: Loro ↔ KDL

### Loro → KDL (write to disk)

Triggered after any committed Loro update, **debounced at 500 ms** after the last change. High-frequency edits (e.g. dragging a mindmap node) do not cause per-frame writes.

1. Reads the schema for this zaplet version.
2. Traverses the Loro doc depth-first.
3. Emits canonical KDL v2 nodes according to the type mapping.
4. For `LoroMap`: emits a named node per entry with properties/children in schema-defined order.
5. For `LoroList`: emits children in list order.
6. For `LoroTree`: emits nested nodes following parent-child links; `xrefs` lists emitted as properties.
7. For `LoroText`: emits as a KDL string argument.
8. Writes the file **atomically** (write to `.tmp`, then rename) and updates the sidecar.

### KDL → Loro (load from disk)

Triggered on daemon startup or on fswatch change.

1. Parse the `zaplet` header to get the schema version.
2. If version < current: run the migration chain (see Migration System).
3. Traverse KDL nodes and populate the Loro doc:
   - Named nodes with an `id=` property → entries in a `LoroMap`.
   - Ordered child nodes → entries in a `LoroList` or `LoroTree`.
   - Leaf values → primitive fields on a `LoroMap`.
   - `#null` values → stored as `null` in the Loro map; absent nodes → key not present.
4. Merge with the sidecar if it exists (see Sidecar section).

---

## Schema Definition (Per Zaplet)

Zaplet schemas are written in the **KDL Schema language** ([kdl-schema crate](https://docs.rs/kdl-schema/latest/kdl_schema/), [spec examples](https://github.com/kdl-org/kdl/blob/main/examples/kdl-schema.kdl)), which provides enum validation, min/max cardinality, and type annotations natively.

The schema serves two roles:
- **Validation:** The daemon validates every incoming Loro update against the schema, rejecting malformed ops before they are applied. This runs on every update; performance can be optimised later.
- **Reification guide:** Declares which Loro paths map to which KDL node names, attribute names, and orderings.

The `(markdown)` type annotation on a field signals that it should be backed by `LoroText` in the Loro doc rather than a plain string.

```kdl
// schema.kdl — expressed in KDL Schema language
// NOTE: KDL Schema v2 compatibility must be verified (see Open Questions §2)
document {
    node zaplet {
        min 1; max 1
        prop version { value { min 1 } }
        prop created {}
        prop author {}
    }
    node task {
        min 0
        prop id { id }
        node title { min 1; max 1; value { (markdown) } }
        node status {
            min 1; max 1
            value { enum { value todo; value in-progress; value done } }
        }
        node due    { min 0; max 1; value { } }   // #null allowed
        node assignee { min 0; max 1; value { } }
        node tags   { min 0; values { } }
        node created { min 1; max 1; value { } }
    }
    node order { min 0; max 1; values { } }
}
```

---

## Migration System

Migrations transform a KDL document from schema version N to N+1. Only KDL-level migrations are supported — there are no Loro-level migrations. This keeps migration logic data-driven, auditable, and language-agnostic.

### Ownership

Each zaplet bundle ships its complete migration chain. Every released version of a zaplet must know how to migrate from **all previous schema versions** of that zaplet. The migration chain is never truncated except by explicit decision (at which point old schema versions become permanently unreadable).

### KDL Migration DSL (proposed)

```kdl
migration from=1 to=2 {
    // v1 had a boolean "done" field; v2 uses a "status" enum
    rename-field  path="task.*" from=done to=status
    map-value     path="task.*.status" {
        true  done
        false todo
    }
}

migration from=2 to=3 {
    // v3 adds a nullable "due" field and a top-level ordering list
    add-field  path="task.*"  name=due    default=#null
    add-node   name=order     type=list   default=""
}
```

### Migration Execution

```text
version_in_file = parse_header(kdl)
current_version = zaplet_schema.version

if version_in_file < current_version:
    backup_sidecar()   // copy .zel to .zel.bak.N before any changes
    for v in version_in_file..current_version:
        kdl = run_migration(kdl, from=v, to=v+1)
    update_header(kdl, version=current_version)
    write_kdl_to_disk(kdl)   // atomic write; reified migrated state
    if migration_failed:
        restore_sidecar_backup()
        return error
```

Migrations run against the KDL representation before loading into Loro. On success the migrated KDL is immediately reified to disk and the sidecar is updated.

### Forward Compatibility

The daemon always serves the current version of each zaplet bundle to clients. Clients connecting to the daemon will always receive the most up-to-date editor. The scenario requiring forward compatibility is a client that has been offline, created a new zaplet doc with a newer schema version locally, and then syncs back — in that case the daemon must accept the newer doc or reject it if it doesn't recognise the schema version (requiring a daemon update first).

---

## Zaplet Bundle Anatomy

```text
~/.config/zlnd/zaplets/
  todo/
    zaplet_todo_3/          ← versioned by schema version
      index.html            ← single-page editor (served to client)
      editor.js             ← compiled JS (loro-wasm + daemon WS)
      editor.css
      schema.kdl            ← KDL Schema definition for this version
      migrations.kdl        ← full chain from v1 → current
      icon.svg              ← shown in "open with" affordance
```

Zaplet bundles are served from a **local directory** (`~/.config/zlnd/zaplets/<name>/`). The daemon watches this directory via fsnotify and picks up new versions automatically without restart. A "reload now for update" banner is shown in the client when a newer zaplet version is detected; the user completes their current editing session before reloading.

Built-in zaplets (todo, mmap, image) are embedded in the daemon binary via `include_dir!` as a fallback, but the local directory takes precedence.

The versioned subdirectory naming convention (`zaplet_todo_3`) reflects the **schema version**, not the editor code version. Multiple schema versions can coexist on disk to support the migration path.

---

## Daemon Routing

```text
GET /zaplet/<subtype>/<filepath>        → serve editor HTML for current schema version
WS  /zaplet/sync/<subtype>/<filepath>  → Loro binary sync channel
GET /zaplet/asset/<subtype>/<asset>    → serve JS/CSS/icon
GET /zaplet/file/<filepath>            → serve referenced binary file (images, etc.)
```

On WebSocket connect:

1. Daemon loads (or creates) the Loro doc for `filepath` (subject to LRU cache, see below).
2. Validates the schema for every incoming Loro op; rejects malformed updates.
3. Sends the full Loro snapshot to the client as binary message.
4. Receives incremental Loro updates from the client; applies them to the doc; queues reification.
5. Broadcasts deltas to all other connected clients for the same file.

### In-Memory Loro Doc Cache

The daemon maintains a config-controlled LRU cache of open Loro docs. When a doc is evicted, its sidecar is flushed. On next access the doc is reloaded from KDL + sidecar. The default limit is configurable in `zlnd.conf` (suggested default: 50 docs).

Cross-doc indexes (e.g. a global tag index across all `.todo.kdl` files in a project) are **not** stored in Loro. They are maintained in an SQLite database within the daemon and rebuilt from source files when needed.

---

## Frontend Integration

### ZapletPane Component

In the Svelte app, `MarkdownPane` detects `.*.kdl` files by extension and renders a `ZapletPane` component instead of a markdown preview.

`ZapletPane`:

1. Fetches `/zaplet/<subtype>/<filepath>` and renders it in a sandboxed `<iframe>`.
2. The iframe's `editor.js` opens `WS /zaplet/sync/<subtype>/<filepath>`.
3. The editor uses `loro-wasm` in the browser to apply and emit updates.
4. All persistence is handled by the daemon; the editor is **stateless across page reloads**.

### Iframe Isolation

The editor iframe is sandboxed (`sandbox="allow-scripts allow-same-origin"`). It communicates with the outer Svelte shell via `postMessage` for:

- Focus / blur events (for keyboard shortcut passthrough)
- Title updates (shown in the pane tab bar)
- Online/offline status (shown as a small icon; never blocks editing)

### Offline-First Behaviour

The client caches zaplet editors locally. All editing is supported without a daemon connection. When offline:

- The editor applies Loro ops to a local copy of the doc.
- Ops are buffered for replay when the daemon reconnects.
- The online/offline status icon reflects connection state; no blocking dialogs.
- `zn new <subtype> <name>` can create a new zaplet doc offline; it is pushed to the daemon on next sync.

### Singleton Pane Invariant

A given `.*.kdl` file may only be open in **one pane** at a time on the client. Attempting to open the same file twice (via file tree and sidebar simultaneously, for example) navigates to the existing pane rather than creating a second one.

### Editor Responsiveness

All zaplet editors must be responsive. They use semantic HTML with minimal custom CSS classes so that they render acceptably on both desktop and Android WebView without a separate mobile code path.

### Creating and Opening Zaplets

| Action | Mechanism |
|--------|-----------|
| Open existing zaplet | File tree in the sidebar, or `zn open <file>` |
| Create new zaplet | `zn new <subtype> <name>` creates `<name>.<subtype>.kdl` in the current project root and opens the editor on the connected remote client; a "New" button with a dropdown of known subtypes is also shown in the sidebar |
| Override to plain text | Not supported. The daemon rejects a `.todo.kdl` file that fails schema validation; it cannot be opened as a raw text editor within zelland. |

---

## Example Zaplets

### 1. Todo List (`work-tasks.todo.kdl`)

**Loro types:** `LoroMap` for task records, `LoroList` for ordering and tags, `LoroText` for the `title` (markdown) field.

**Editor:** Checklist UI with drag-to-reorder. Inline editing of title, due date picker, assignee autocomplete from daemon's known project contributors.

**KDL reification:**

```kdl
zaplet todo version=3 created="2026-02-22T10:00:00Z" author=njr
task id=k8f2a {
    title "Fix the login bug"
    status in-progress
    due "2026-03-15"
    assignee njr
    tags backend auth
    created "2026-02-20T09:00:00Z"
}
order k8f2a m3x9p
```

### 2. Mindmap Canvas (`brainstorm.mmap.kdl`)

**Loro types:** `LoroTree` for node hierarchy, `LoroMap` for canvas viewport and per-node metadata, `LoroText` for node labels (markdown typed).

**Editor:** Canvas with drag-to-create, click-to-edit, drag-to-reparent. Real-time multi-user cursors via ephemeral awareness (not persisted in KDL). Cross-links rendered as dashed arrows between nodes.

**KDL reification:**

```kdl
zaplet mmap version=1 created="2026-02-22T11:00:00Z" author=njr
canvas offset-x=0.0 offset-y=0.0 zoom=1.0
node id=root label="Project Alpha" color="#7aa2f7" {
    node id=n1 label=Frontend color="#9ece6a" {
        node id=n3 label=Components color="#e0af68" {}
        node id=n4 label=State      color="#e0af68" {}
    }
    node id=n2 label=Backend color="#f7768e" {
        node id=n5 label=API  color="#bb9af7" {}
        node id=n6 label=Auth color="#bb9af7" xrefs=n3 {}
    }
}
```

### 3. Image Feature Markup (`ui-mockup.image.kdl`)

**Loro types:** `LoroMap` for annotation index, `LoroList` for z-order, `LoroText` for labels (markdown typed). Image data is a relative URI; the daemon serves the file.

**Editor:** Image viewer with overlay drawing tools (rect, ellipse, arrow, freehand). Click annotation to expand comment thread (reuses the annotation model from `ANNOTATION_DESIGN.md`).

**KDL reification:**

```kdl
zaplet image version=1 created="2026-02-22T12:00:00Z" author=njr
image src="./mockups/dashboard.png" width=1440 height=900
annotation id=a1b2c shape=rect {
    bounds x=120.0 y=45.0 w=320.0 h=180.0
    label "Login form — needs better error states"
    author njr
    color "#f7768e"
    created "2026-02-22T12:05:00Z"
}
order a1b2c d4e5f
```

---

## Open Questions

The following questions remain unresolved and must be answered before implementation begins.

### 1. Three-Way KDL Diff Algorithm

When a `.*.kdl` file is edited externally while the daemon holds an in-memory Loro doc, the daemon must merge them via three-way diff. The "base" is the last reified KDL state. Questions:

- Is the base stored inside the sidecar (as a KDL snapshot alongside the Loro binary), or is it re-derived from the Loro doc at the time of the last successful reification?
- What is the diff algorithm? Line-based three-way merge (like `git merge`) is simple but structurally unaware. A node-semantic KDL diff (match nodes by `id=` property, diff children) would be more correct but requires a custom implementation.
- What happens when the diff produces a genuine conflict (same node deleted on one side and modified on the other)? Does the Loro state win, the KDL edit win, or does the daemon surface a conflict to the user?

### 2. Zaplet Binary Format

The local zaplet directory hosts editor bundles, but the exact format is undecided: **static HTML/JS/CSS directory**, **WASM blob**, or **Rust shared library (`.so`)**?

- A static directory is the simplest: the daemon serves `index.html` and assets via HTTP. No ABI is needed. Zaplet logic runs entirely in the browser (JS + `loro-wasm`). Schema validation in the daemon is done by the daemon's own compiled Rust code (per built-in zaplet) or by evaluating the KDL Schema file.
- A `.so` / WASM plugin would allow the zaplet to contribute Rust validation logic, custom reification, and migration code to the daemon. This is more powerful but requires a stable ABI and a plugin loading system.
- The hybrid approach: static HTML directory for the editor, plus an optional WASM component for daemon-side validation and migration logic.

This decision determines the entire plugin architecture. **Needs resolution before any implementation.**

### 3. Ephemeral State Boundary

Zoom level, canvas pan offset, and cursor position are per-session data that should survive page reload but not be committed to the KDL file or shared with other clients as persistent state.

Options:
- Store ephemeral state **client-side only** (localStorage / sessionStorage). Simple but lost on cache clear.
- Store it via **Loro's awareness channel** (ephemeral, not snapshotted). Shared across connected clients in real time but not persisted.
- Store it in a **separate small KDL block** in the sidecar (e.g. a `client-prefs` section), keyed by client identity.

The schema needs a way to mark fields as ephemeral so the reifier skips them.

### 4. Enum Representation Conventions in KDL v2

KDL v2 has no native enum type. The schema (KDL Schema language) can define an enum with `value { enum { value todo; value in-progress; ... } }`, but the KDL on disk is just a bare identifier (`status in-progress`). Questions:

- Should enum values always be bare identifiers (restricting allowed chars to identifier-safe ones)?
- If an enum value contains spaces or starts with a digit, must it be quoted? Does the schema DSL need to document this constraint?
- How does the editor know to render a `status` field as a dropdown vs a free-text input? Is this encoded in the schema, or does the editor define its own UI hints separately?

### 5. Domain Invariants and Manual-Merge Fields

Some fields have application-level constraints that Loro cannot enforce (e.g. a task cannot simultaneously be `todo` and `done`; a calendar event cannot have `end` before `start`). Concurrent offline edits can violate these.

- Should the schema support marking fields as `conflict-sensitive` (or similar)? On merge, the daemon flags these for manual resolution rather than auto-merging.
- What does "manual merge required" look like in the editor UX? A warning banner? A dedicated conflict-resolution modal?
- Does the daemon surface the conflict as a Tauri event, leaving resolution to the specific zaplet's editor UI?

### 6. Author Identity

Each Loro operation needs to be tagged with an author ID for attribution and awareness. The daemon has an active SSH session, but:

- Multiple people can share an SSH session via the same Unix user. Is `username@hostname` sufficient, or do we need an out-of-band identity layer?
- For offline clients (no daemon connection), what author ID is used for locally buffered ops? A locally cached identity string set during first sync?
- How is author identity displayed in the editor (e.g. comment threads, "last edited by")?

### 7. KDL Schema Crate — KDL v2 Compatibility

The `kdl-schema` crate ([docs.rs](https://docs.rs/kdl-schema/latest/kdl_schema/)) and the reference KDL Schema spec ([kdl-org/kdl](https://github.com/kdl-org/kdl/blob/main/examples/kdl-schema.kdl)) need to be verified for:

- Full KDL v2 syntax support (`#true`, `#false`, `#null`, bare identifiers).
- Ability to express `(markdown)` as a custom type annotation that the daemon maps to `LoroText`.
- Whether the crate exposes a Rust validation API suitable for per-op validation in the daemon's WebSocket handler.

### 8. Offline Client Caching Mechanism

The client caches zaplet editors for offline use. The caching mechanism is unspecified:

- **Service Worker + Cache API**: intercepts fetch for zaplet assets, serves from cache when offline. Standard web approach; works in WebKit.
- **Tauri asset embedding**: zaplet bundles embedded in the app binary at build time. Zero runtime caching needed, but requires app rebuild to update zaplets.
- **Hybrid**: built-in zaplets embedded in binary; user-installed zaplets cached via Service Worker.

The choice affects how `zn new <subtype>` works offline (can only create docs for cached/embedded subtypes).

### 9. `zn new` Remote Targeting

`zn new todo work-tasks` should create the stub KDL and open the zaplet editor on the "connected remote client". Which client? In a multi-session setup (multiple open zelland windows), which one receives the open command? Should there be a default target (e.g. most recently active session), or should `zn new` require an explicit `--session` flag?
