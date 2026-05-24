#!/usr/bin/env node

import { mkdir, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";

const version = process.env.KATEX_VERSION || "0.16.25";
const baseUrl =
  process.env.KATEX_URL ||
  `https://cdn.jsdelivr.net/npm/katex@${version}/dist`;
const files = [
  "katex.min.css",
  "katex.min.js",
  "fonts/KaTeX_AMS-Regular.woff2",
  "fonts/KaTeX_Caligraphic-Bold.woff2",
  "fonts/KaTeX_Caligraphic-Regular.woff2",
  "fonts/KaTeX_Fraktur-Bold.woff2",
  "fonts/KaTeX_Fraktur-Regular.woff2",
  "fonts/KaTeX_Main-Bold.woff2",
  "fonts/KaTeX_Main-BoldItalic.woff2",
  "fonts/KaTeX_Main-Italic.woff2",
  "fonts/KaTeX_Main-Regular.woff2",
  "fonts/KaTeX_Math-BoldItalic.woff2",
  "fonts/KaTeX_Math-Italic.woff2",
  "fonts/KaTeX_SansSerif-Bold.woff2",
  "fonts/KaTeX_SansSerif-Italic.woff2",
  "fonts/KaTeX_SansSerif-Regular.woff2",
  "fonts/KaTeX_Script-Regular.woff2",
  "fonts/KaTeX_Size1-Regular.woff2",
  "fonts/KaTeX_Size2-Regular.woff2",
  "fonts/KaTeX_Size3-Regular.woff2",
  "fonts/KaTeX_Size4-Regular.woff2",
  "fonts/KaTeX_Typewriter-Regular.woff2",
];

for (const file of files) {
  const url = `${baseUrl}/${file}`;
  const target = resolve("assets/katex", file);
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`failed to fetch KaTeX ${version} ${file}: HTTP ${response.status}`);
  }

  const bytes = new Uint8Array(await response.arrayBuffer());
  await mkdir(dirname(target), { recursive: true });
  await writeFile(target, bytes);
  console.log(`wrote ${target} from ${url}`);
}
