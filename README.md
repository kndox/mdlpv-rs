# mdlpv-rs

Neovim Markdown live preview powered by a small Rust localhost server.

[日本語版 README](README.ja.md)

The Neovim plugin sends the current Markdown buffer to the Rust server, the server renders it to HTML, and a browser viewer updates through SSE.

## Requirements

- Rust toolchain
- Neovim with `vim.system()` support
- `curl`
- A desktop browser opener such as `xdg-open`, `open`, or Windows `start`
- Chrome, Chromium, Edge, or another compatible headless browser for PDF export

## Build

```bash
cargo build --release
```

## Install

Build the Rust server and put it somewhere on `PATH`:

```bash
cargo install --path .
```

For local development, `cargo build --release` is enough. Point `server_bin` at `target/release/mdlpv-rs`, or keep `mdlpv-rs` on `PATH`.

Install the Neovim runtime files with your plugin manager by pointing it at this repository. For a manual checkout, add this repository to Neovim's runtime path:

```lua
vim.opt.runtimepath:append("/path/to/mdlpv-rs")
require("mdlive").setup()
```

### Lazy.nvim

For a local checkout managed by Lazy.nvim:

```lua
{
  dir = "/path/to/mdlpv-rs",
  build = "cargo build --release",
  config = function(plugin)
    local binary = plugin.dir .. "/target/release/mdlpv-rs"
    if vim.fn.has("win32") == 1 then
      binary = binary .. ".exe"
    end

    require("mdlive").setup({
      server_bin = binary,
      mermaid = {
        enabled = true,
        mode = "local",
      },
    })
  end,
}
```

For a GitHub install with Lazy.nvim, use:

```lua
{
  "kndox/mdlpv-rs",
  build = function(plugin)
    loadfile(plugin.dir .. "/lua/mdlive/install.lua")().install(plugin)
  end,
  config = function(plugin)
    local binary = plugin.dir .. "/mdlpv-rs"
    if vim.fn.has("win32") == 1 then
      binary = binary .. ".exe"
    end

    require("mdlive").setup({
      server_bin = binary,
      open_browser = true,
      scroll_sync = true,
      server_log_level = "warn",
      server_stderr_notify = false,
      mermaid = {
        enabled = true,
        mode = "local",
        cdn_url = "https://cdn.jsdelivr.net/npm/mermaid@11.15.0/dist/mermaid.min.js",
      },
      pdf_browser_cmd = nil,
      pdf_extra_args = {},
    })
  end,
}
```

This downloads the matching prebuilt `mdlpv-rs` binary from the latest GitHub Release and verifies it with `SHA256SUMS`. It requires `curl` plus a SHA-256 tool (`sha256sum`, `shasum`, or PowerShell). Use the local checkout example above if you want Lazy.nvim to build the server from source with a Rust toolchain.

## Neovim Setup

```lua
require("mdlive").setup({
  server_bin = "mdlpv-rs",
  host = "127.0.0.1",
  port = 0,
  server_log_level = "warn",
  server_stderr_notify = false,
  debounce_ms = 250,
  scroll_sync = true,
  scroll_debounce_ms = 50,
  open_browser = true,
  pdf_browser_cmd = nil,
  pdf_virtual_time_budget_ms = 5000,
  pdf_extra_args = {},
  mermaid = {
    enabled = true,
    mode = "local",
    cdn_url = "https://cdn.jsdelivr.net/npm/mermaid@11.15.0/dist/mermaid.min.js",
  },
})
```

Mermaid modes:

- `local`: use the bundled offline Mermaid asset.
- `cdn`: load Mermaid from `mermaid.cdn_url`.
- `none`: disable Mermaid rendering.
- `local-with-cdn-fallback`: try the bundled asset first, then fall back to the CDN URL.

Commands:

- `:MdLiveStart`
- `:MdLiveStop`
- `:MdLiveOpen`
- `:MdLiveRestart`
- `:MdLiveExportHtml`
- `:MdLiveExportPdf`

On normal Neovim exit, `VimLeavePre` sends SIGTERM to the server process. If Neovim is killed forcibly, cleanup cannot run and the server process may remain.

## Mermaid

Mermaid is loaded only when a Markdown document contains a `mermaid` fenced code block.

To bundle the local offline asset:

```bash
node scripts/fetch-mermaid.mjs
```

This writes `assets/mermaid/mermaid.min.js`. The default version is `11.15.0`; override it with `MERMAID_VERSION`.

## Math

Inline and display math are rendered with the bundled KaTeX browser assets. To refresh those local offline assets:

```bash
node scripts/fetch-katex.mjs
```

This writes `assets/katex/`. The default version is `0.16.25`; override it with `KATEX_VERSION`.

## Images and Export

Relative Markdown image paths are served from the Markdown file's parent directory. Paths outside that directory are rejected.

Use `:MdLiveExportHtml` to save the current preview as HTML.
Use `:MdLiveExportPdf` to print the exported HTML through a headless browser and save it as PDF.

## License / Third-party Notices

`mdlpv-rs` is licensed under the MIT License; see `LICENSE`.

This repository bundles `mermaid@11.15.0` and `katex@0.16.25` for offline rendering. Both are licensed under the MIT License; see `THIRD_PARTY_NOTICES.md`.

## Safety

The server binds to `127.0.0.1` by default. Raw HTML in Markdown is escaped before rendering in v0.1.

## Development

- Contributing: `CONTRIBUTING.md`
- Test strategy: `docs/testing.md` / `docs/testing.ja.md`
- Design notes: `docs/design.md` / `docs/design.ja.md`
- Release packaging: `docs/release.md` / `docs/release.ja.md`
- Roadmap: `docs/roadmap.md` / `docs/roadmap.ja.md`
