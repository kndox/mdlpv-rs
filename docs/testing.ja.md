# テスト

## Rust

変更前後に以下を実行する。

```bash
cargo fmt --check
cargo test
cargo clippy -- -D warnings
NVIM_LOG_FILE=/tmp/mdlive-nvim.log nvim --headless -u NONE -n -l tests/lua/mdlive_spec.lua
```

Rust の test は Markdown render、安全な raw HTML の render と sanitize、Mermaid fence 検出、session revision 更新、HTTP session API を対象にする。

## Neovim Plugin

手動 smoke test:

1. `cargo build --release` で server を build する。
2. この repository を `runtimepath` に入れて Neovim を起動する。
3. `server_bin` に `target/release/mdlpv-rs` を指定する。
4. Markdown buffer を開いて `:MdLiveStart` を実行する。
5. browser が開き、初回 preview が表示されることを確認する。
6. buffer を編集し、preview が更新されることを確認する。
7. `mermaid` fenced block を追加し、その document だけで Mermaid が lazy load されることを確認する。
8. `:MdLiveExportHtml` を実行し、HTML file が書き出されることを確認する。
9. Chrome/Chromium 互換の headless browser が入った環境で `:MdLiveExportPdf` を実行し、PDF file が書き出されることを確認する。
10. `:MdLiveStop` を実行し、server process が終了することを確認する。

Lua plugin test は headless Neovim 上で実行し、server/curl/headless browser 呼び出し用に `vim.system()` を stub する。command 登録、startup、request payload、HTML export、PDF export command 構築、process cleanup を検証する。
