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

`.github/workflows/audit.yml` runs `cargo audit` on pushes, pull requests, and weekly (Monday 00:00 UTC). It can also be started manually.

`.github/workflows/release.yml` builds x86_64 and arm64 archives for Linux, macOS, and Windows when a `v*` tag is pushed. The workflow first runs the Rust and Lua checks, then publishes a GitHub Release with those archives, `SHA256SUMS`, and build provenance attestations.

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

## Upstream Tracking

### Mermaid math labels (RaTeX / `ttf-parser`)

Decision (2026-11-22): merman's optional `math` feature is disabled, so RaTeX and `ttf-parser` are not linked. Mermaid diagrams whose labels contain `$$...$$` show their source and an error (``render session lacks capability `math` required by diagram …``). Markdown body math is unaffected: it renders in the browser with the bundled KaTeX.

Why:

- `ttf-parser` is unmaintained: [RUSTSEC-2026-0192](https://rustsec.org/advisories/RUSTSEC-2026-0192). The maintainer wrote in [harfbuzz/ttf-parser#217](https://github.com/harfbuzz/ttf-parser/issues/217) that the library is "currently unmaintained/abandoned"; the same issue mentions an unfixed excessive-CPU report. The advisory suggests [`skrifa`](https://crates.io/crates/skrifa) (fontations) as the replacement.
- The only path to `ttf-parser` was merman's `math` feature → `merman-render` → RaTeX (`ratex-svg`, `ratex-font-loader`, `ratex-unicode-font`) → `ttf-parser` 0.25, directly and via `fontdb` 0.23 and `ab_glyph`/`owned_ttf_parser`.
- merman 0.11.0 made `math` opt-in, so the default features no longer include RaTeX.

Upstream status:

| Item | Last checked | Status | Where to check |
| --- | --- | --- | --- |
| RaTeX drops `ttf-parser` (e.g. moves to `skrifa`) | 2026-11-22 | Not started. `ratex-unicode-font` 0.1.16 (latest) depends on `ttf-parser` 0.25 and `fontdb` 0.23; `ratex-svg`/`ratex-font-loader` depend on `ab_glyph` 0.2. No issue or PR found. | [crates.io](https://crates.io/crates/ratex-unicode-font), [issues: ttf-parser](https://github.com/erweixin/RaTeX/issues?q=ttf-parser), [issues: skrifa](https://github.com/erweixin/RaTeX/issues?q=skrifa) |
| `ab_glyph` drops `ttf-parser` | 2026-11-22 | 0.2.32 (latest) depends on `owned_ttf_parser` 0.25. | [crates.io](https://crates.io/crates/ab_glyph) |
| merman tracks the advisory or changes its math backend | 2026-11-22 | 0.11.0: `math` is opt-in and still uses RaTeX. No issue found. | [releases](https://github.com/Latias94/merman/releases), [issues: ttf-parser](https://github.com/Latias94/merman/issues?q=ttf-parser) |
| `ttf-parser` maintenance or a vulnerability advisory | 2026-11-22 | Unmaintained; no patched version. | [#217](https://github.com/harfbuzz/ttf-parser/issues/217), [RustSec](https://rustsec.org/packages/ttf-parser.html) |

How to re-check:

```bash
# Latest versions
cargo search ratex-unicode-font --limit 1
cargo search merman --limit 1
# Does enabling math still pull in ttf-parser? (in a temporary copy)
tmp=$(mktemp -d) && cp Cargo.toml Cargo.lock "$tmp" && mkdir "$tmp/src" && echo 'fn main() {}' > "$tmp/src/main.rs" && (cd "$tmp" && cargo add merman --features math && cargo tree -i ttf-parser)
```

Re-enable math labels when `cargo tree -i ttf-parser` shows no match with `features = ["math"]`: set `merman = { version = "...", features = ["math"] }`, add the RaTeX and KaTeX font notices from the new crates to `docs/third-party/merman/`, update this section, README, and the release notes. Update the "Last checked" column whenever you check. Opening an issue on RaTeX to request the migration is an option; it has not been done.

### merman 0.11.0 crate notices omit some components

| Item | Last checked | Status | Where to check |
| --- | --- | --- | --- |
| `merman-render` notices list `non-layered-tidy-tree-layout` (still translated in `src/mindmap/tidy_tree.rs`) | 2026-11-22 | Missing in 0.11.0; carried over from 0.9.2 in `docs/third-party/merman/supplemental/`. | `merman-render-<version>/THIRD_PARTY_NOTICES.md` |
| ELK/elkjs license files shipped with the ELK crates | 2026-11-22 | 0.11.0 ships only `EPL-2.0.txt`; the 0.9.2 Eclipse ELK and elkjs license files are kept in `supplemental/`. | `merman-elk-layered-<version>/LICENSES/` |

When upgrading merman, drop a supplemental file once the new crate notices cover it.

## Remaining Work

- Verify the first public release on GitHub before announcing the prebuilt Lazy.nvim install path.

## 0.3.0

Security hardening for the local server. Session API calls require a per-launch token, requests with unexpected `Host` headers are rejected, the image API serves only image files, and the CSP no longer allows inline or remote scripts. Update the plugin and server binary together. PDF export keeps the browser sandbox; add `--no-sandbox` to `pdf_extra_args` if needed. Also fixes stale state after `:MdLiveRestart`, duplicate sessions from repeated `:MdLiveStart`, out-of-order updates, and image names containing `%`. Mermaid now renders with merman 0.11.0; math inside Mermaid labels is no longer supported (see [Upstream Tracking](#upstream-tracking)), while Markdown body math is unchanged. Source builds use the latest stable Rust toolchain.

## 0.2.1

Security update for dependencies flagged by `cargo audit`: ammonia 4.2.5 (HTML/SVG/MathML sanitization XSS fixes), pulldown-cmark 0.13.6, anyhow 1.0.104, and tokio 1.54.1. No configuration changes.

## 0.2.0

Mermaid rendering now uses merman 0.9.2. Update the plugin and server binary together. Lua `mermaid.mode` / `mermaid.cdn_url` are removed; `mermaid.enabled` remains. CLI modes are `merman` / `none`. Source builds require Rust 1.95 or later.

`docs/third-party/merman/` contains license texts and notices for linked components, including ELK and fonts. Keep this directory in release archives; the existing packaging workflow includes it by copying all of `docs/`.
