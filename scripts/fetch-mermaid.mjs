#!/usr/bin/env node

import { mkdir, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";

const version = process.env.MERMAID_VERSION || "11.15.0";
const target = resolve("assets/mermaid/mermaid.min.js");
const url =
  process.env.MERMAID_URL ||
  `https://cdn.jsdelivr.net/npm/mermaid@${version}/dist/mermaid.min.js`;

const response = await fetch(url);
if (!response.ok) {
  throw new Error(`failed to fetch Mermaid ${version}: HTTP ${response.status}`);
}

const source = await response.text();
if (!source.includes("mermaid")) {
  throw new Error("downloaded file does not look like Mermaid");
}

await mkdir(dirname(target), { recursive: true });
await writeFile(target, source);
console.log(`wrote ${target} from ${url}`);
