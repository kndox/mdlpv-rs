# リリース

## Versioning

release tag を作る前に `Cargo.toml` の version を更新する。tag は `vX.Y.Z` 形式にする。

## Package contents

release artifact には以下を含める。

- `mdlpv-rs` server binary
- `lua/` と `plugin/` の Neovim runtime files
- `assets/` viewer files
- `README.md`
- `LICENSE`
- `docs/`
- `THIRD_PARTY_NOTICES.md`

## CI

`.github/workflows/ci.yml` は push と pull request で format、test、clippy を実行する。

`.github/workflows/release.yml` は `v*` tag push 時に Linux、macOS、Windows 向け archive を build し、それらの archive と `SHA256SUMS` を GitHub Release に公開する。

## Repository operations

`main` への変更は pull request 経由にする。外部 contribution を広く受ける前に、`main` に branch protection を設定する。

- merge 前に CI success を必須にする。
- force push を禁止する。
- contributor pull request は squash merge を基本にする。
- binary download policy が決まるまで、release publishing は maintainer-owned にする。

## Public readiness

repository を public にする前に以下を確認する。

- `Cargo.toml` に公開用 package metadata と正しい repository URL が入っている。
- `README.md` と `README.ja.md` の GitHub install 例が `kndox/mdlpv-rs` を使っている。
- `LICENSE` が存在し、bundled dependency の notice は `THIRD_PARTY_NOTICES.md` に残っている。
- `CONTRIBUTING.md`、`SECURITY.md`、issue templates、pull request template が存在する。
- `cargo fmt --check`、`cargo test`、`cargo clippy -- -D warnings`、headless Neovim Lua plugin test が通る。

## Remaining work

- 最初の public release を GitHub 上で確認してから、prebuilt Lazy.nvim install path を告知する。
