# Third-Party Notices

This project includes third-party software for native diagram rendering and offline browser math.

## Merman

- Package: `merman@0.9.2`, with default SVG, all-diagram, Cytoscape, ELK, and math features.
- Source: <https://github.com/Latias94/merman/tree/v0.9.2>
- License: MIT OR Apache-2.0; the MIT option is reproduced in [LICENSE-MIT](docs/third-party/merman/LICENSE-MIT).
- Dependency attributions, exact provenance and license texts: [Merman notices](docs/third-party/merman/NOTICE.md).

### Eclipse ELK source availability

The binary links the unmodified `merman-elk-layered@0.9.2` Rust translation of Eclipse ELK, licensed under EPL-2.0. Its license and attribution are included in the Merman notices above. The corresponding translated source is available from [crates.io](https://crates.io/api/v1/crates/merman-elk-layered/0.9.2/download) and [the v0.9.2 source tree](https://github.com/Latias94/merman/tree/v0.9.2/crates/merman-elk-layered). The original Eclipse ELK derivation is revision `62d5909f96fad541bc101ad52dabaece6b7eab7e`; the elkjs derivation is revision `a8304cf79fde75bc2ab1a89d28320f53f8637436`.

### Embedded fonts

Merman's RaTeX math support includes KaTeX font data under SIL Open Font License 1.1. The font notice and full OFL text are included in the Merman notices. These fonts are separate from the browser KaTeX software below.

## KaTeX

- Package: `katex@0.16.25`
- Source: <https://github.com/KaTeX/KaTeX>
- Bundled files: `assets/katex/`
- License: MIT

```text
The MIT License (MIT)

Copyright (c) 2013-2020 Khan Academy and other contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
