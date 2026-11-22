# mdlpv-rs

Neovim Markdown live preview powered by a small Rust localhost server.

[日本語版 README](README.ja.md)

The Neovim plugin sends the current Markdown buffer to the Rust server, the server renders it to HTML, and a browser viewer updates through SSE.

## Requirements

- Latest stable Rust toolchain
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
      },
      pdf_browser_cmd = nil,
      pdf_extra_args = {},
    })
  end,
}
```

This downloads the matching prebuilt `mdlpv-rs` binary and verifies it with `SHA256SUMS`. When the plugin checkout is at a release tag (for example with Lazy.nvim `version` or `tag`), the binary from that release is used; otherwise the latest release is used. `SHA256SUMS` detects corrupted downloads but comes from the same release; to verify provenance, run `gh attestation verify <archive> --repo kndox/mdlpv-rs`. It requires `curl` plus a SHA-256 tool (`sha256sum`, `shasum`, or PowerShell). Use the local checkout example above if you want Lazy.nvim to build the server from source with a Rust toolchain.

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
  },
})
```

Mermaid diagrams render in Rust using merman. Set `mermaid.enabled = false` to disable rendering.

Commands:

- `:MdLiveStart`
- `:MdLiveStop`
- `:MdLiveOpen`
- `:MdLiveRestart`
- `:MdLiveExportHtml`
- `:MdLiveExportPdf`

PDF export keeps the headless browser sandbox enabled. If your environment requires it (for example running as root in a container), add `"--no-sandbox"` to `pdf_extra_args`.

On normal Neovim exit, `VimLeavePre` sends SIGTERM to the server process, which closes open viewers and shuts down. If Neovim is killed forcibly, cleanup cannot run and the server process may remain.

## Mermaid

Mermaid fenced code blocks render on the server with merman 0.11.0 as embedded SVG images. They need no Mermaid JavaScript or CDN connection, and the same images are included in HTML/PDF exports.

Malformed diagrams and resource limit failures show their source and an error in that block. Each diagram uses merman's default resource limits and a five-second cooperative deadline. HTML labels are converted to SVG text; interactive links and JavaScript actions within diagrams are unavailable. Rendering is not guaranteed to match Mermaid.js exactly.

Math inside Mermaid labels (`$$...$$`) is not supported: merman's optional RaTeX math renderer is disabled because it depends on the unmaintained `ttf-parser` crate. Such diagrams show their source and an error. Math in the Markdown body is unaffected and still renders with KaTeX in the browser. See [Upstream Tracking](docs/release.md#upstream-tracking) for the status of re-enabling it.

### Migrating to 0.3.0

- Update the plugin and server binary together. The server now requires a per-launch token for session API calls, so older plugin versions cannot drive a 0.3.0 server.
- PDF export no longer passes `--no-sandbox` by default. Add it to `pdf_extra_args` if your environment needs it.
- Mermaid now renders with merman 0.11.0. Math inside Mermaid labels (`$$...$$`) is no longer supported; Markdown body math is unchanged.

### Migrating to 0.2.0

- Keep `mermaid.enabled`; remove `mode` and `cdn_url`. Legacy options trigger one migration warning and rendering follows `enabled`. Replace legacy `mode = "none"` with `enabled = false`.
- CLI `--mermaid-mode` now accepts `merman` (default) or `none`. `--mermaid-cdn-url` has been removed.
- Update the plugin and server binary together. The Mermaid.js distribution and fetch script are no longer needed.

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

Mermaid rendering uses merman; browser math uses bundled `katex@0.16.25`. See `THIRD_PARTY_NOTICES.md` for licenses, including ELK and embedded fonts.

## Safety

The server binds to `127.0.0.1` by default. Safe raw HTML in Markdown is rendered after sanitization. This includes elements such as `<details><summary>More</summary>Text</details>`, `<font color="red">red</font>`, and static document elements including `main`, `section`, `address`, `tfoot`, `meter`, and `progress`. The deprecated `font` element supports only its `color` attribute.

Inline `style` attributes support a limited set of text, color, spacing, border, size, overflow, and basic flex layout properties. For example, `<span style="color: red; font-weight: bold">important</span>` is supported. Properties that can load external resources, overlay the viewer, or otherwise affect content outside the normal document flow are removed. This includes `background-image`, `position`, `z-index`, `content`, `transform`, `filter`, `cursor`, and `pointer-events`. Dangerous tags, event handler attributes, unsafe URLs, and other unsupported attributes are also removed.

## Development

- Contributing: `CONTRIBUTING.md`
- Test strategy: `docs/testing.md` / `docs/testing.ja.md`
- Design notes: `docs/design.md` / `docs/design.ja.md`
- Release packaging: `docs/release.md` / `docs/release.ja.md`
- Roadmap: `docs/roadmap.md` / `docs/roadmap.ja.md`
