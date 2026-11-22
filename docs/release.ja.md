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

`.github/workflows/audit.yml` は push、pull request、毎週月曜 00:00 UTC に `cargo audit` を実行する。手動実行も可能。

`.github/workflows/release.yml` は `v*` tag push 時に Linux、macOS、Windows 向け archive を build し、それらの archive と `SHA256SUMS` を GitHub Release に公開する。公開前に Rust と Lua の検査を実行し、archive には build provenance attestation を付ける。

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

## Upstream tracking

### Mermaid の数式ラベル（RaTeX／`ttf-parser`）

判断（2026-11-22）: merman の optional な `math` feature を無効にし、RaTeX と `ttf-parser` をリンクしない。ラベルに `$$...$$` を含む Mermaid 図は、ソースとエラー（``render session lacks capability `math` required by diagram …``）を表示する。Markdown 本文の数式は影響を受けず、同梱の KaTeX により browser 側で描画する。

理由:

- `ttf-parser` は unmaintained である: [RUSTSEC-2026-0192](https://rustsec.org/advisories/RUSTSEC-2026-0192)。maintainer は [harfbuzz/ttf-parser#217](https://github.com/harfbuzz/ttf-parser/issues/217) で library が "currently unmaintained/abandoned" であると書いており、同 issue では CPU を過剰に消費する問題の未修正の報告にも触れられている。advisory は代替として [`skrifa`](https://crates.io/crates/skrifa)（fontations）を挙げている。
- `ttf-parser` への経路は、merman の `math` feature → `merman-render` → RaTeX（`ratex-svg`、`ratex-font-loader`、`ratex-unicode-font`）→ `ttf-parser` 0.25 のみだった。直接依存のほか、`fontdb` 0.23 経由と `ab_glyph`／`owned_ttf_parser` 経由がある。
- merman 0.11.0 で `math` は opt-in になり、default feature には RaTeX が含まれなくなった。

上流の状況:

| 項目 | 最終確認日 | 状況 | 確認先 |
| --- | --- | --- | --- |
| RaTeX が `ttf-parser` をやめる（`skrifa` への移行など） | 2026-11-22 | 未着手。`ratex-unicode-font` 0.1.16（最新）は `ttf-parser` 0.25 と `fontdb` 0.23 に、`ratex-svg`／`ratex-font-loader` は `ab_glyph` 0.2 に依存。issue／PR はない。 | [crates.io](https://crates.io/crates/ratex-unicode-font)、[issues: ttf-parser](https://github.com/erweixin/RaTeX/issues?q=ttf-parser)、[issues: skrifa](https://github.com/erweixin/RaTeX/issues?q=skrifa) |
| `ab_glyph` が `ttf-parser` をやめる | 2026-11-22 | 0.2.32（最新）は `owned_ttf_parser` 0.25 に依存。 | [crates.io](https://crates.io/crates/ab_glyph) |
| merman が advisory に対応する、または数式 backend を変える | 2026-11-22 | 0.11.0: `math` は opt-in で、引き続き RaTeX を使う。issue はない。 | [releases](https://github.com/Latias94/merman/releases)、[issues: ttf-parser](https://github.com/Latias94/merman/issues?q=ttf-parser) |
| `ttf-parser` の maintenance 再開、または脆弱性 advisory | 2026-11-22 | unmaintained。patched version はない。 | [#217](https://github.com/harfbuzz/ttf-parser/issues/217)、[RustSec](https://rustsec.org/packages/ttf-parser.html) |

再確認の方法:

```bash
# 最新 version
cargo search ratex-unicode-font --limit 1
cargo search merman --limit 1
# math を有効にすると ttf-parser が入るか（一時コピーで確認）
tmp=$(mktemp -d) && cp Cargo.toml Cargo.lock "$tmp" && mkdir "$tmp/src" && echo 'fn main() {}' > "$tmp/src/main.rs" && (cd "$tmp" && cargo add merman --features math && cargo tree -i ttf-parser)
```

`features = ["math"]` の状態で `cargo tree -i ttf-parser` が何も返さなくなったら、数式ラベルを再有効化する。`merman = { version = "...", features = ["math"] }` とし、新しい crate から RaTeX と KaTeX font の notice を `docs/third-party/merman/` に追加し、この節、README、release note を更新する。確認したら「最終確認日」を更新する。RaTeX に移行を求める issue を立てる選択肢もあるが、まだ行っていない。

### merman 0.11.0 の crate notice に一部の component がない

| 項目 | 最終確認日 | 状況 | 確認先 |
| --- | --- | --- | --- |
| `merman-render` の notice に `non-layered-tidy-tree-layout` が載る（`src/mindmap/tidy_tree.rs` で引き続き翻訳されている） | 2026-11-22 | 0.11.0 では欠落。0.9.2 のものを `docs/third-party/merman/supplemental/` に残している。 | `merman-render-<version>/THIRD_PARTY_NOTICES.md` |
| ELK 系 crate に同梱される ELK／elkjs の license file | 2026-11-22 | 0.11.0 は `EPL-2.0.txt` のみ同梱。0.9.2 の Eclipse ELK と elkjs の license file を `supplemental/` に残している。 | `merman-elk-layered-<version>/LICENSES/` |

merman を更新するときは、新しい crate の notice で網羅された supplemental file を削除する。

## Remaining work

- 最初の public release を GitHub 上で確認してから、prebuilt Lazy.nvim install path を告知する。

## 0.3.0

local server のセキュリティ強化。session API は起動ごとの token を必須とし、想定外の `Host` header を持つ request を拒否し、image API は画像 file のみを返し、CSP は inline script と remote script を許可しなくなった。plugin と server binary は同時に更新する。PDF export は browser の sandbox を維持し、必要な場合は `pdf_extra_args` に `--no-sandbox` を追加する。あわせて `:MdLiveRestart` 後の状態破損、`:MdLiveStart` の繰り返しによる session 重複、update の順序逆転、`%` を含む画像名の扱いを修正した。Mermaid は merman 0.11.0 で描画し、Mermaid のラベル内の数式は非対応とした（[Upstream tracking](#upstream-tracking) を参照）。Markdown 本文の数式は変わらない。source build は最新の stable Rust toolchain を前提とする。

## 0.2.1

`cargo audit` で検出された依存関係の脆弱性を修正。ammonia 4.2.5(HTML／SVG／MathML サニタイズの XSS 修正)、pulldown-cmark 0.13.6、anyhow 1.0.104、tokio 1.54.1 に更新しました。設定の変更はありません。

## 0.2.0

Mermaid 描画を merman 0.9.2 に統一。プラグインとサーバーバイナリを同時に更新してください。Lua の `mermaid.mode`／`mermaid.cdn_url` は廃止し、`mermaid.enabled` を維持します。CLI の mode は `merman`／`none` です。ビルドには Rust 1.95 以上が必要です。

`docs/third-party/merman/` はバイナリ内の ELK とフォントを含むライセンス本文・通知を保持します。既存のアーカイブ作成処理が `docs/` 全体を同梱するため、この directory も必ず配布してください。
