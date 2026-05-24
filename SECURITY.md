# Security Policy

## Supported Versions

The current `main` branch and the latest tagged release are the supported targets for security fixes.

## Reporting a Vulnerability

Please do not open a public GitHub issue for a security vulnerability.

Use GitHub private vulnerability reporting if it is enabled for this repository. If it is not available, contact the maintainer through their GitHub profile and include only the minimum details needed to establish a private reporting channel.

## Security Scope

`mdlpv-rs` is local preview tooling. By default, the server binds to `127.0.0.1` and is intended to be used from the same machine as Neovim.

Security-relevant reports include:

- Cases where the server unexpectedly exposes content outside the intended local preview scope.
- Path traversal or file disclosure issues.
- Markdown, Mermaid, math, or export behavior that enables script execution beyond the documented safety model.
- Unsafe defaults that expose the preview server to other hosts.

Reports about rendering differences from GitHub-flavored Markdown are usually feature requests unless they create a security impact.
