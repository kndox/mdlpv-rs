# Design Notes

## Goal

`mdlpv-rs` previews the current Neovim Markdown buffer through a small Rust localhost server and a browser viewer.

The first public version intentionally avoids WebView, Electron, and Tauri. It uses an existing browser, full-buffer POST updates, full Markdown re-rendering, and DOM replacement in the viewer.

## Components

```text
Neovim Lua plugin
  -> creates and updates sessions through the Rust server
  -> opens the preview URL in a browser

Rust server
  -> renders Markdown
  -> stores sessions in memory
  -> serves viewer assets
  -> sends update and scroll events through SSE

Browser viewer
  -> fetches rendered HTML
  -> subscribes to session events
  -> replaces the preview DOM
  -> lazy-loads Mermaid only when needed
```

## Public Interfaces

Neovim commands:

- `:MdLiveStart`
- `:MdLiveStop`
- `:MdLiveOpen`
- `:MdLiveRestart`
- `:MdLiveExportHtml`
- `:MdLiveExportPdf`

Server defaults:

- `--host 127.0.0.1`
- `--port 0`
- `--mermaid-mode local`
- `--log-level info`

The server prints one startup JSON line to stdout after binding:

```json
{"host":"127.0.0.1","port":53123}
```

Logs are written to stderr.

## HTTP Surface

The Neovim plugin and viewer use these server routes:

- `GET /health`
- `POST /api/session`
- `POST /api/session/{session_id}`
- `POST /api/session/{session_id}/scroll`
- `DELETE /api/session/{session_id}`
- `GET /api/rendered/{session_id}`
- `GET /api/export/{session_id}`
- `GET /events/{session_id}`
- `GET /view/{session_id}`
- `GET /assets/viewer.js`
- `GET /assets/style.css`
- `GET /assets/mermaid/mermaid.min.js`

Sessions are per Markdown buffer and are stored only in memory.

## Rendering

Markdown is rendered with `pulldown-cmark` using tables, footnotes, strikethrough, and task lists.

Mermaid is not rendered by Rust. The server detects Mermaid fenced code blocks and returns `has_mermaid`. The viewer then converts Mermaid code blocks and loads Mermaid according to the configured mode.

Inline and display math are rendered in the browser with bundled KaTeX assets.

## Security Stance

The default bind address is `127.0.0.1`, and the tool is intended for local preview use.

Raw HTML in Markdown is escaped before rendering. Relative Markdown images are served from the Markdown file's parent directory, and paths outside that directory are rejected.

Mermaid runs in the browser with `securityLevel: "strict"`.

## Non-goals for v0.1

- Bundled WebView.
- Electron or Tauri.
- Bidirectional editing.
- Complete GitHub-compatible rendering.
- Incremental updates.
