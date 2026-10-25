# Architecture

## Overview

The plugin has three layers: a Neovim Lua plugin, a Rust server, and a browser viewer.

- The Lua plugin owns user commands, server process management, buffer POSTs, and browser opening.
- The Rust server owns HTTP APIs, session storage, Markdown rendering, SSE, and asset serving.
- The browser viewer owns rendered HTML fetching, SSE subscription, DOM replacement, and display of server-rendered Mermaid images.

## Data Flow

1. `:MdLiveStart` starts the Rust server.
2. The server prints the bound host and port as one JSON line on stdout.
3. The Lua plugin sends the buffer content with `POST /api/session`.
4. The server renders Markdown to HTML and stores the session.
5. The Lua plugin opens the returned `view_url`.
6. The viewer fetches `/api/rendered/{session_id}` and displays it.
7. On buffer changes, the Lua plugin debounces and posts to `/api/session/{id}`.
8. The server increments the revision and emits an SSE `update` event.
9. The viewer fetches the latest rendered HTML and replaces the content.
10. On cursor movement, the Lua plugin debounces and posts to `/api/session/{id}/scroll`, and the viewer scrolls to the source line anchor after an SSE `scroll` event.

## Session Model

Sessions are per Markdown buffer and are stored only in memory for v0.1. A session contains:

- `id`
- `path`
- `title`
- source `content`
- rendered HTML
- `has_mermaid`
- `revision`
- update broadcast channel

## Mermaid

The server renders Mermaid fenced code blocks with merman as embedded SVG images. `has_mermaid` remains fence-detection metadata, including when rendering is disabled.

Rendering runs on blocking workers with at most two jobs at once. Updates are serialized per session; rendering does not hold the session-map lock. The HTML sanitizer runs before server-generated placeholders are replaced with SVG data images. Diagram failures display escaped source and diagnostics without failing the document.

At startup, the server uses the `assets` directory next to its executable for KaTeX. If that directory does not exist, it uses `assets` in the build-time Cargo manifest directory for source builds. Once a directory is selected, missing files are not loaded from another directory. Keep the packaged `assets` directory alongside the executable when moving a release installation.

## Security Stance

v0.1 is localhost-only preview tooling. The default bind address is `127.0.0.1`. Raw HTML in Markdown is escaped during rendering.
