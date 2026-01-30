# Contributing

We welcome contributions to OpenLumen Node.

## Getting started

1. **Build the project** — Follow [Installation](docs/installation.md) to install prerequisites and build the firmware and web UI.
2. **Make your changes** — Use a branch and keep commits focused.
3. **Open a pull request** — Describe what you changed and why; reference any issues.

## Development

- **Firmware:** Rust (Embassy). The repo uses the toolchain in [rust-toolchain.toml](rust-toolchain.toml). Run `cargo build --release` after building the web frontend (see [Installation](docs/installation.md)).
- **Web UI:** `web_frontend/` is a Vite + React app. Use `npm run dev` for local development; the firmware embeds the output of `npm run build:client`.
- **CI** — GitHub Actions builds the web frontend and firmware on push/PR; see [.github/workflows/ci.yml](.github/workflows/ci.yml).

## Questions and issues

Open an issue for bugs, feature ideas, or questions. For security-sensitive topics, consider contacting the maintainers privately if needed.

Thanks for contributing.
