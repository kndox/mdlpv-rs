# Roadmap

This roadmap tracks public follow-up work after the v0.1 preview workflow. It is not a commitment to a specific release date.

## Current Focus

- Verify the prebuilt Lazy.nvim install path on the first public GitHub Release.
- Keep the localhost server default safe and predictable.
- Maintain test coverage for Markdown rendering, session updates, export commands, and Neovim process cleanup.

## Planned Work

- Verify GitHub Release publishing after the repository is public and CI is verified on GitHub.

## Candidates

These may be considered after the public release path is stable:

- More GitHub-compatible Markdown rendering.
- Incremental preview updates.
- Bidirectional editing experiments.
- Additional hardening for rendered HTML and third-party diagram rendering.

## Completed

- Rust localhost server with Markdown session APIs.
- Browser viewer with SSE live reload.
- Neovim Lua plugin commands and process management.
- Mermaid lazy loading with a bundled local asset option.
- KaTeX-based math rendering with bundled assets.
- Relative image serving with directory containment checks.
- HTML and PDF export commands.
- CI and release package workflows.
- Lazy.nvim prebuilt binary installer with SHA-256 verification.
