# Release

## Versioning

Update `Cargo.toml` before tagging a release. Release tags use `vX.Y.Z`.

## Package Contents

Release artifacts contain:

- `mdlpv-rs` server binary
- `lua/` and `plugin/` Neovim runtime files
- `assets/` viewer files
- `README.md`
- `LICENSE`
- `docs/`
- `THIRD_PARTY_NOTICES.md`

## CI

`.github/workflows/ci.yml` runs formatting, tests, and clippy on pushes and pull requests.

`.github/workflows/release.yml` builds x86_64 and arm64 archives for Linux, macOS, and Windows when a `v*` tag is pushed. The workflow publishes a GitHub Release with those archives and `SHA256SUMS`.

## Repository Operations

Use pull requests for changes to `main`. Configure branch protection for `main` before broad public contribution:

- Require CI to pass before merge.
- Disallow force pushes.
- Prefer squash merge for contributor pull requests.
- Keep release publishing maintainer-owned until the binary download policy is finalized.

## Public Readiness

Before making the repository public:

- Confirm `Cargo.toml` has public package metadata and the correct repository URL.
- Confirm `README.md` and `README.ja.md` use `kndox/mdlpv-rs` in GitHub install examples.
- Confirm `LICENSE` exists and the bundled dependency notices remain in `THIRD_PARTY_NOTICES.md`.
- Confirm `CONTRIBUTING.md`, `SECURITY.md`, issue templates, and the pull request template exist.
- Run `cargo fmt --check`, `cargo test`, `cargo clippy -- -D warnings`, and the headless Neovim Lua plugin test.

## Remaining Work

- Verify the first public release on GitHub before announcing the prebuilt Lazy.nvim install path.

## 0.2.0

Mermaid rendering now uses merman 0.9.2. Update the plugin and server binary together. Lua `mermaid.mode` / `mermaid.cdn_url` are removed; `mermaid.enabled` remains. CLI modes are `merman` / `none`. Source builds require Rust 1.95 or later.

`docs/third-party/merman/` contains license texts and notices for linked components, including ELK and fonts. Keep this directory in release archives; the existing packaging workflow includes it by copying all of `docs/`.
