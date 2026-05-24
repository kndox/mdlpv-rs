# 設計メモ

## Goal

`mdlpv-rs` は、Neovim で編集中の Markdown buffer を小さな Rust localhost server と browser viewer で preview する。

最初の公開版では WebView、Electron、Tauri は使わない。既存 browser を使い、Markdown は全文 POST、全文再 render、viewer 側の DOM 差し替えで更新する。

## Components

```text
Neovim Lua plugin
  -> Rust server へ session 作成・更新 request を送る
  -> preview URL を browser で開く

Rust server
  -> Markdown を render する
  -> session を memory に保持する
  -> viewer assets を serve する
  -> SSE で update / scroll event を送る

Browser viewer
  -> rendered HTML を fetch する
  -> session event を subscribe する
  -> preview DOM を差し替える
  -> Mermaid が必要な時だけ lazy-load する
```

## Public interfaces

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

server は bind 後に stdout へ startup JSON を 1 行出す。

```json
{"host":"127.0.0.1","port":53123}
```

log は stderr に出す。

## HTTP surface

Neovim plugin と viewer は以下の server routes を使う。

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

session は Markdown buffer ごとに作られ、memory のみに保持される。

## Rendering

Markdown rendering には `pulldown-cmark` を使い、tables、footnotes、strikethrough、task lists を有効にする。

Mermaid は Rust 側では render しない。server は Mermaid fenced code block を検出して `has_mermaid` を返す。viewer は Mermaid code block を変換し、設定された mode に従って Mermaid を読み込む。

inline math と display math は bundled KaTeX assets を使って browser 側で render する。

## Security stance

default bind address は `127.0.0.1` であり、local preview 用途を前提にする。

Markdown 内の raw HTML は render 前に escape する。relative Markdown image は Markdown file の parent directory から serve し、その directory 外の path は拒否する。

Mermaid は browser 側で `securityLevel: "strict"` を指定する。

## v0.1 non-goals

- bundled WebView。
- Electron / Tauri。
- bidirectional editing。
- complete GitHub-compatible rendering。
- incremental updates。
