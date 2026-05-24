# Testing

## Rust

Run the standard checks before sending changes:

```bash
cargo fmt --check
cargo test
cargo clippy -- -D warnings
NVIM_LOG_FILE=/tmp/mdlive-nvim.log nvim --headless -u NONE -n -l tests/lua/mdlive_spec.lua
```

The Rust tests cover Markdown rendering, raw HTML escaping, Mermaid fence detection, session revision updates, and the HTTP session API.

## Neovim Plugin

Manual smoke test:

1. Build the server with `cargo build --release`.
2. Start Neovim with this repository on `runtimepath`.
3. Configure `server_bin` to `target/release/mdlpv-rs`.
4. Open a Markdown buffer and run `:MdLiveStart`.
5. Confirm the browser opens and renders the first preview.
6. Edit the buffer and confirm the preview updates.
7. Add a `mermaid` fenced block and confirm Mermaid loads only for that document.
8. Run `:MdLiveExportHtml` and confirm an HTML file is written.
9. Run `:MdLiveExportPdf` with a Chrome/Chromium-compatible headless browser installed and confirm a PDF file is written.
10. Run `:MdLiveStop` and confirm the server process exits.

The Lua plugin test runs in headless Neovim and stubs `vim.system()` for server, curl, and headless browser calls. It covers command registration, startup, request payloads, HTML export, PDF export command construction, and process cleanup.
