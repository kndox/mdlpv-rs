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
  -> displays SVG images rendered by merman
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
- `--mermaid-mode merman`
- `--log-level info`

The server prints one startup JSON line to stdout after binding:

```json
{"host":"127.0.0.1","port":53123,"token":"<random>"}
```

`token` is generated per server launch. The session mutation routes (`POST`/`DELETE` under `/api/session`) require `Authorization: Bearer <token>`. The Lua plugin stores the header in a private temporary file and passes it to curl with `--header @file`, so the token does not appear in process arguments.

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
- `GET /api/image/{session_id}/{path}`
- `GET /assets/viewer.js`
- `GET /assets/style.css`
- `GET /assets/export.js`
- `GET /assets/katex/{path}`

Sessions are per Markdown buffer and are stored only in memory.

## Rendering

Markdown is rendered with `pulldown-cmark` using tables, footnotes, strikethrough, and task lists.

Mermaid fenced code blocks are rendered in Rust using merman and embedded as SVG images. `--mermaid-mode none` preserves the source as code; `has_mermaid` remains detection metadata. The viewer and HTML/PDF export share the rendered images.

Inline and display math are rendered in the browser with bundled KaTeX assets.

## Security Stance

The default bind address is `127.0.0.1`, and the tool is intended for local preview use.

Requests must carry a `Host` header matching the bound address (loopback binds also accept `localhost`, `127.0.0.1`, and `[::1]`), which blocks DNS rebinding from web pages. Wildcard binds such as `0.0.0.0` skip this check; session mutation still requires the token. Viewer and export routes are guarded by the unguessable session ID.

The viewer and export pages run no inline scripts; the CSP allows scripts only from the server itself.

Raw HTML in Markdown is rendered through an allowlist-based sanitizer. Dangerous tags, event handler attributes, and unsafe URLs are removed. Relative Markdown images are served from the Markdown file's parent directory, and paths outside that directory are rejected. Only image file extensions are served, and image responses use a sandboxing CSP so SVG files cannot run scripts. Relative URLs inside raw HTML are not rewritten to the image API.

Merman uses strict security settings, finite default resource limits, and a five-second cooperative deadline per diagram. Generated SVG is embedded as an image after Markdown sanitization, without permitting arbitrary raw SVG or data URLs. Errors show escaped source and diagnostics in the affected block.

## Non-goals for v0.1

- Bundled WebView.
- Electron or Tauri.
- Bidirectional editing.
- Complete GitHub-compatible rendering.
- Incremental updates.
