# NOTES

- Use PLAN.md to track all milestones and tasks. Update tasks once completed.

- Always check-in your incremental work using jj. Use `jj init git` if needed to start a repository.

- Always look for tests in the canonical format for the languages you're writing and make sure they pass after successful compilation. Make sure TESTING.md is up to date with details about tests implemented and planned.

## New Feature Process

- Always create a feature_DESIGN.md in ./docs/features before starting. Consider whether new types of testing is required.

- Break down the feature into easily completed tasks, add them to a new section in project_root/PLAN.md, and keep the status updated as you build.

## Project Layout

  This is a monorepo with three main components:
`daemon-rs/` — Rust daemon (axum/tokio), the backend server (`zlnd` + `zn` CLI)
`src-tauri/` — Rust Tauri v2 client, bridges the daemon API to the Svelte UI
`src/` — Svelte 5 frontend (SvelteKit, xterm.js, yjs)

## Build System

Use `task` (Taskfile.yml) for all build/test/deploy commands — not raw cargo/npm. You might need to invoke `go-task` binary on this platform.
`task test` runs all test suites (cargo test for both Rust crates + npm test for Svelte).
`task dev` launches daemon + Tauri dev mode in a Zellij session.
`task build` produces release binaries for Linux + Android.

## Rust Conventions (daemon-rs and src-tauri)

Protobuf wire format via `prost 0.14.3` — pinned in both crates for compatibility. The single proto file is `proto/zelland.proto` at the project root.
Config files use KDL format (kdl crate v6). See memory notes for API gotchas.
YJS/CRDT sync uses `yrs 0.21` directly with manual lib0 framing — do not add `y-sync` as a dependency.
Async runtime is tokio; HTTP framework is axum 0.8.
Tests: unit tests in `#[cfg(test)]` modules, integration tests use `axum::ServiceExt::oneshot()` or bind to `127.0.0.1:0`.
Use `tracing` for logging, not `println!` or `log`.

## Svelte Conventions (src/)

Svelte 5 with runes (`$state`, `$derived`, `$effect`) — do not use legacy stores or `$:` reactive statements.
Component tests use vitest + @testing-library/svelte.
Styling: Pico CSS + minimal custom classes in custom.css.

## Secrets & Security

Never commit `.env*` files, `*.keystore`, `google-services.json`, or SSH keys.
The daemon's trigger endpoints are guarded by loopback-only middleware — preserve this pattern for any new endpoints that accept commands.

## Proto / API Changes

Edit `proto/zelland.proto` — both crates compile from this single file.
WebSocket messages are binary protobuf, not JSON. REST endpoints use JSON.

# context-mode — MANDATORY routing rules

You have context-mode MCP tools available. These rules are NOT optional — they protect your context window from flooding. A single unrouted command can dump 56 KB into context and waste the entire session.

## BLOCKED commands — do NOT attempt these

### curl / wget — BLOCKED
Any shell command containing `curl` or `wget` will be intercepted and blocked by the context-mode plugin. Do NOT retry.
Instead use:
- `context-mode_ctx_fetch_and_index(url, source)` to fetch and index web pages
- `context-mode_ctx_execute(language: "javascript", code: "const r = await fetch(...)")` to run HTTP calls in sandbox

### Inline HTTP — BLOCKED
Any shell command containing `fetch('http`, `requests.get(`, `requests.post(`, `http.get(`, or `http.request(` will be intercepted and blocked. Do NOT retry with shell.
Instead use:
- `context-mode_ctx_execute(language, code)` to run HTTP calls in sandbox — only stdout enters context

### Direct web fetching — BLOCKED
Do NOT use any direct URL fetching tool. Use the sandbox equivalent.
Instead use:
- `context-mode_ctx_fetch_and_index(url, source)` then `context-mode_ctx_search(queries)` to query the indexed content

## REDIRECTED tools — use sandbox equivalents

### Shell (>20 lines output)
Shell is ONLY for: `git`, `mkdir`, `rm`, `mv`, `cd`, `ls`, `npm install`, `pip install`, and other short-output commands.
For everything else, use:
- `context-mode_ctx_batch_execute(commands, queries)` — run multiple commands + search in ONE call
- `context-mode_ctx_execute(language: "shell", code: "...")` — run in sandbox, only stdout enters context

### File reading (for analysis)
If you are reading a file to **edit** it → reading is correct (edit needs content in context).
If you are reading to **analyze, explore, or summarize** → use `context-mode_ctx_execute_file(path, language, code)` instead. Only your printed summary enters context.

### grep / search (large results)
Search results can flood context. Use `context-mode_ctx_execute(language: "shell", code: "grep ...")` to run searches in sandbox. Only your printed summary enters context.

## Tool selection hierarchy

1. **GATHER**: `context-mode_ctx_batch_execute(commands, queries)` — Primary tool. Runs all commands, auto-indexes output, returns search results. ONE call replaces 30+ individual calls.
2. **FOLLOW-UP**: `context-mode_ctx_search(queries: ["q1", "q2", ...])` — Query indexed content. Pass ALL questions as array in ONE call.
3. **PROCESSING**: `context-mode_ctx_execute(language, code)` | `context-mode_ctx_execute_file(path, language, code)` — Sandbox execution. Only stdout enters context.
4. **WEB**: `context-mode_ctx_fetch_and_index(url, source)` then `context-mode_ctx_search(queries)` — Fetch, chunk, index, query. Raw HTML never enters context.
5. **INDEX**: `context-mode_ctx_index(content, source)` — Store content in FTS5 knowledge base for later search.

## Output constraints

- Keep responses under 500 words.
- Write artifacts (code, configs, PRDs) to FILES — never return them as inline text. Return only: file path + 1-line description.
- When indexing content, use descriptive source labels so others can `search(source: "label")` later.

## ctx commands

| Command | Action |
|---------|--------|
| `ctx stats` | Call the `stats` MCP tool and display the full output verbatim |
| `ctx doctor` | Call the `doctor` MCP tool, run the returned shell command, display as checklist |
| `ctx upgrade` | Call the `upgrade` MCP tool, run the returned shell command, display as checklist |
