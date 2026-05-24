# ロードマップ

このロードマップは v0.1 の preview workflow 以降の公開向け follow-up を管理するものです。特定の release date を約束するものではありません。

## Current focus

- 最初の public GitHub Release で prebuilt Lazy.nvim install path を確認する。
- localhost server の default を安全で予測しやすい状態に保つ。
- Markdown rendering、session update、export command、Neovim process cleanup の test coverage を維持する。

## Planned work

- repository を public にし、GitHub 上で CI が通ることを確認してから GitHub Release 公開を確認する。

## Candidates

public release path が安定した後、以下を検討する。

- より GitHub-compatible な Markdown rendering。
- incremental preview updates。
- bidirectional editing の実験。
- rendered HTML と third-party diagram rendering の追加 hardening。

## Completed

- Markdown session API を持つ Rust localhost server。
- SSE live reload に対応した browser viewer。
- Neovim Lua plugin commands と process management。
- bundled local asset option 付き Mermaid lazy loading。
- bundled assets を使う KaTeX-based math rendering。
- directory containment check 付き relative image serving。
- HTML/PDF export commands。
- CI と release package workflows。
- SHA-256 検証付き Lazy.nvim prebuilt binary installer。
