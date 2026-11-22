# Merman 0.11.0 third-party notices

mdlpv-rs links `merman@0.11.0` with its default features (`all-diagrams`, `svg`, `layout-cytoscape`, `layout-elk`). The optional `math` feature (RaTeX) is not enabled, so RaTeX and its embedded KaTeX fonts are not linked.

The notices and license texts below are copied unmodified from the published 0.11.0 crates that the binary links. Each `THIRD_PARTY_NOTICES.md` lists that crate's translated, copied, or embedded components and points to its `THIRD_PARTY_LICENSES/` files.

| Crate | Notices | License of the crate |
| --- | --- | --- |
| `merman-core@0.11.0` | [notices](merman-core/THIRD_PARTY_NOTICES.md) | MIT OR Apache-2.0 |
| `merman-render@0.11.0` | [notices](merman-render/THIRD_PARTY_NOTICES.md) | MIT OR Apache-2.0 |
| `merman-model@0.11.0` | [notices](merman-model/THIRD_PARTY_NOTICES.md) | MIT OR Apache-2.0 |
| `merman-markup@0.11.0` | [notices](merman-markup/THIRD_PARTY_NOTICES.md) | MIT OR Apache-2.0 |
| `manatee@0.11.0` | [notices](manatee/THIRD_PARTY_NOTICES.md) | MIT OR Apache-2.0 |
| `dugong@0.11.0` | [notices](dugong/THIRD_PARTY_NOTICES.md) | MIT OR Apache-2.0 |
| `dugong-graphlib@0.11.0` | [notices](dugong-graphlib/THIRD_PARTY_NOTICES.md) | MIT OR Apache-2.0 |
| `merman-unicode-linebreak@0.11.0` | [notices](merman-unicode-linebreak/THIRD_PARTY_NOTICES.md) | Apache-2.0 ([license](merman-unicode-linebreak/LICENSE)) |
| `merman-elk-layered@0.11.0` | Eclipse ELK translation, see below | EPL-2.0 ([license](merman-elk-layered/EPL-2.0.txt)) |
| `roughr-merman@0.12.6` | — | MIT ([license](roughr-merman/LICENSE)) |

The remaining merman crates (`merman`, `merman-layout-elk`, `merman-theme-contract`) are MIT OR Apache-2.0 without separate third-party notices; the MIT text is in [LICENSE-MIT](LICENSE-MIT).

## Eclipse ELK

`merman-elk-layered` is a source-backed Rust translation of Eclipse ELK's layered layout, licensed under EPL-2.0. Its derivation points are Eclipse ELK revision `62d5909f96fad541bc101ad52dabaece6b7eab7e` and elkjs revision `a8304cf79fde75bc2ab1a89d28320f53f8637436`. The corresponding source is available from [crates.io](https://crates.io/api/v1/crates/merman-elk-layered/0.11.0/download).

## Supplemental notices

These license files are carried over from the merman 0.9.2 notices because the 0.11.0 crate notices do not list them, although the code still uses them:

- [non-layered-tidy-tree-layout](supplemental/non-layered-tidy-tree-layout/LICENSE) 2.0.2 (MIT): translated in `merman-render/src/mindmap/tidy_tree.rs`.
- [Eclipse ELK](supplemental/eclipse-elk/LICENSE.md) 0.9.1 and [elkjs](supplemental/elkjs/LICENSE.md) 0.9.3 (EPL-2.0): translation sources and behavior reference of `merman-elk-layered` and `merman-layout-elk`.

## Updating these files

When upgrading merman, replace each crate directory with the `THIRD_PARTY_NOTICES.md` and `THIRD_PARTY_LICENSES/` from the new published crate (`~/.cargo/registry/src/*/<crate>-<version>/`), and re-check the linked crates with `cargo tree -e normal --prefix none | sort -u | grep -E '^(merman|manatee|dugong|roughr)'`.
