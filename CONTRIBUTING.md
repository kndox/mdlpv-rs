# Contributing

Thanks for your interest in improving `mdlpv-rs`.

This project is maintained as a lightweight OSS project. Issues and pull requests are welcome, but the maintainer may keep the scope narrow to preserve a small, reliable Markdown preview tool.

## Before You Start

- Check existing issues and pull requests before opening a new one.
- For larger behavior changes, open an issue first so the direction can be discussed.
- Keep changes focused. Avoid unrelated refactors in the same pull request.

## Development

Required tools:

- Rust toolchain
- Neovim with `vim.system()` support
- `curl`

Run these checks before sending a pull request:

```bash
cargo fmt --check
cargo test
cargo clippy -- -D warnings
NVIM_LOG_FILE=/tmp/mdlive-nvim.log nvim --headless -u NONE -n -l tests/lua/mdlive_spec.lua
```

For manual plugin testing, see `docs/testing.md`.

## Pull Requests

Pull requests should include:

- A clear description of the user-visible change or bug fix.
- Tests or manual verification notes.
- Documentation updates when behavior, commands, options, or install steps change.

The preferred merge style is squash merge. The final merge decision and release timing are maintained by the project maintainer.

## Documentation

- Keep `README.md` focused on installation and everyday usage.
- Put deeper design, testing, and release details under `docs/`.
- When adding or changing files under `docs/`, keep English and Japanese versions in sync where a Japanese version exists.

## Security

Do not report security issues in a public issue. See `SECURITY.md`.
