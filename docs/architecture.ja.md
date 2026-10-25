# アーキテクチャ

## 概要

この plugin は Neovim Lua plugin、Rust server、browser viewer の 3 層で構成する。

- Lua plugin は user command、server process 管理、buffer 内容の POST、browser 起動だけを担当する。
- Rust server は HTTP API、session 管理、Markdown render、SSE、asset 配信を担当する。
- Browser viewer は rendered HTML の fetch、SSE 購読、DOM 差し替え、サーバーで描画した Mermaid 画像の表示を担当する。

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

Mermaid fenced code block は server 側で merman を使って SVG 画像に描画する。`has_mermaid` は描画無効時も含めた fence の検出情報として維持する。

描画は blocking worker で同時に最大 2 件まで実行する。更新は session 単位で直列化し、描画中は session 一覧の lock を保持しない。HTML sanitize 後に、内部生成した placeholder のみを SVG data 画像へ置き換える。図の失敗時は文書全体を失敗させず、エスケープ済みのソースとエラーを表示する。

server は起動時に、実行ファイルの隣にある `assets` directory を KaTeX の配信元として選ぶ。その directory が存在しない場合は、source build 用にコンパイル時の Cargo manifest directory 内の `assets` を使う。選択した directory にファイルが欠けていても別の directory には切り替えない。配布版を移動する場合は、同梱の `assets` directory も実行ファイルの隣に配置する。

## Security stance

v0.1 は localhost preview 専用であり、default は `127.0.0.1` bind とする。Markdown 内の raw HTML は render 時に text として escape する。
