# mdlpv-rs

小さな Rust localhost server で動く Neovim Markdown live preview plugin です。

[English README](README.md)

Neovim plugin が現在の Markdown buffer を Rust server に送り、server が HTML に render します。browser viewer は SSE で更新されます。

## Requirements

- Rust toolchain 1.95+
- `vim.system()` を使える Neovim
- `curl`
- `xdg-open`、`open`、Windows `start` などの desktop browser opener
- PDF export 用の Chrome、Chromium、Edge、または互換 headless browser

## Build

```bash
cargo build --release
```

## Install

Rust server を build し、`PATH` 上に置きます。

```bash
cargo install --path .
```

local development では `cargo build --release` で十分です。`server_bin` に `target/release/mdlpv-rs` を指定するか、`mdlpv-rs` を `PATH` 上に置いてください。

Neovim runtime files は、この repository を plugin manager から指定して install します。手動 checkout の場合は、この repository を Neovim の runtime path に追加します。

```lua
vim.opt.runtimepath:append("/path/to/mdlpv-rs")
require("mdlive").setup()
```

### Lazy.nvim

Lazy.nvim で local checkout を使う例です。

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

Lazy.nvim で GitHub から install する場合は、以下の形になります。

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

この方式では Lazy.nvim が GitHub Releases の latest から環境に合う prebuilt `mdlpv-rs` binary を download し、`SHA256SUMS` で検証します。`curl` と SHA-256 計算用 command（`sha256sum`、`shasum`、または PowerShell）が必要です。Rust toolchain で source から build したい場合は、上の local checkout 例を使ってください。

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

Mermaid は Rust の merman で描画します。`mermaid.enabled = false` で無効化できます。

Commands:

- `:MdLiveStart`
- `:MdLiveStop`
- `:MdLiveOpen`
- `:MdLiveRestart`
- `:MdLiveExportHtml`
- `:MdLiveExportPdf`

通常の Neovim 終了時は `VimLeavePre` で server process に SIGTERM を送り、停止します。Neovim が強制終了された場合は cleanup が走らないため、server process が残る可能性があります。

## Mermaid

Mermaid fenced code block は merman 0.9.2 でサーバー側で SVG に描画し、画像として埋め込みます。JavaScript の読み込みや CDN 接続は不要で、HTML／PDF エクスポートにも同じ図が含まれます。

構文エラーやリソース制限超過時は、該当ブロックにソースとエラーを表示します。各図には merman の既定リソース制限と 5 秒の協調的な期限を適用します。HTML ラベルは SVG テキストに変換され、図のリンクや JavaScript による操作は利用できません。Mermaid.js と完全に同じ描画を保証するものではありません。

### 0.2.0 への移行

- Lua 設定の `mermaid.enabled` は引き続き使用できます。`mode` と `cdn_url` は削除してください。旧設定を検出した場合は一度だけ通知し、`enabled` に従って動作します。旧 `mode = "none"` は `enabled = false` に置き換えてください。
- CLI の `--mermaid-mode` は `merman`（既定）と `none` に変更しました。`--mermaid-cdn-url` は廃止しました。
- プラグインとサーバーバイナリを一緒に更新してください。Mermaid.js の配布ファイルと取得スクリプトは不要になりました。

## Math

inline math と display math は bundled KaTeX browser assets で render されます。local offline assets を更新するには以下を実行します。

```bash
node scripts/fetch-katex.mjs
```

`assets/katex/` が書き出されます。default version は `0.16.25` です。`KATEX_VERSION` で上書きできます。

## Images and Export

relative Markdown image path は Markdown file の parent directory から serve されます。その directory 外の path は拒否されます。

`:MdLiveExportHtml` で現在の preview を HTML として保存します。
`:MdLiveExportPdf` で exported HTML を headless browser から print し、PDF として保存します。

## License / Third-party Notices

`mdlpv-rs` は MIT License です。詳細は `LICENSE` を参照してください。

Mermaid 描画には merman を使用し、数式用に `katex@0.16.25` を同梱します。ELK とフォントを含むライセンス情報は `THIRD_PARTY_NOTICES.md` を参照してください。

## Safety

server は default で `127.0.0.1` に bind します。Markdown 内の安全な raw HTML は sanitize 後に render されます。たとえば `<details><summary>詳細</summary>本文</details>`、`<font color="red">赤字</font>`、`main`、`section`、`address`、`tfoot`、`meter`、`progress` などの静的な文書 element を使用できます。非推奨の `font` element では `color` attribute だけをサポートします。

inline `style` attribute では、文字、色、余白、境界線、サイズ、overflow、基本的な flex layout に関する property を限定的に使用できます。たとえば `<span style="color: red; font-weight: bold">重要</span>` を使用できます。外部 resource の読み込み、viewer への重ね合わせ、通常の document flow 外への影響につながる property は除去されます。これには `background-image`、`position`、`z-index`、`content`、`transform`、`filter`、`cursor`、`pointer-events` が含まれます。危険な tag、event handler attribute、安全でない URL、その他の未対応 attribute も除去されます。

## Development

- Contributing: `CONTRIBUTING.md`
- Test strategy: `docs/testing.md` / `docs/testing.ja.md`
- Design notes: `docs/design.md` / `docs/design.ja.md`
- Release packaging: `docs/release.md` / `docs/release.ja.md`
- Roadmap: `docs/roadmap.md` / `docs/roadmap.ja.md`
