# アーキテクチャ

## 概要

この plugin は Neovim Lua plugin、Rust server、browser viewer の 3 層で構成する。

- Lua plugin は user command、server process 管理、buffer 内容の POST、browser 起動だけを担当する。
- Rust server は HTTP API、session 管理、Markdown render、SSE、asset 配信を担当する。
- Browser viewer は rendered HTML の fetch、SSE 購読、DOM 差し替え、Mermaid lazy load を担当する。

## Data flow

1. `:MdLiveStart` で Lua plugin が Rust server を起動する。
2. Rust server は bind 後、実 port を stdout の JSON で返す。
3. Lua plugin は `POST /api/session` で buffer 内容を送信する。
4. Rust server は Markdown を HTML に render し、session を保存する。
5. Lua plugin は `view_url` を browser で開く。
6. Viewer は `/api/rendered/{session_id}` を fetch して表示する。
7. buffer 変更時、Lua plugin は debounce 後に `POST /api/session/{id}` を送る。
8. Rust server は revision を増やし、SSE `update` event を送る。
9. Viewer は event 受信後に rendered HTML を再 fetch して差し替える。
10. cursor 移動時、Lua plugin は debounce 後に `POST /api/session/{id}/scroll` を送り、viewer は SSE `scroll` event で source line anchor へ移動する。

## Session model

session は Markdown buffer 単位で作る。server 側の session は以下を持つ。

- `id`
- `path`
- `title`
- `content`
- rendered HTML
- `has_mermaid`
- `revision`
- update broadcast channel

v0.1 では process 内 memory のみで保持し、永続化しない。

## Mermaid

Mermaid は server 側では描画しない。Rust server は Mermaid fenced code block の有無だけを判定し、viewer に `has_mermaid` として返す。

viewer は `has_mermaid=true` の場合だけ Mermaid JS を読み込む。default は local asset だが、`assets/mermaid/mermaid.min.js` が未配置なら 404 とし、viewer に明示的なエラーを表示する。local asset は `node scripts/fetch-mermaid.mjs` で取得できる。

## Security stance

v0.1 は localhost preview 専用であり、default は `127.0.0.1` bind とする。Markdown 内の raw HTML は render 時に text として escape する。
