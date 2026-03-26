# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

- No changes yet.

## [0.2.1] - 2026-03-26

### Fixed
- Updated `zinc-core` dependency pin to `=0.1.1` for compatibility with current CLI features.
- Removed local `path` dependency override for release packaging so `cargo package`/`cargo publish` resolve from crates.io in CI.

## [0.2.0] - 2026-03-26

### Added
- ANSI logo asset and branded `version` command output.

### Changed
- `inscription list` now shows newest received inscriptions first.
- `offer` commands remain available via `zinc-cli offer ...` but are hidden from top-level help output.
- Improved human-output inscription thumbnail rendering fidelity in terminal-friendly output modes.

### Docs
- Aligned README/usage/contract/schema docs with the current CLI surface, including `--agent` output mode and thumbnail toggles (`--thumb`, `--no-thumb`).

## [0.1.1] - 2026-03-21

### Fixed
- Removed `image` `avif-native` feature from optional `ui` build path to avoid `dav1d` system-library failures in CI and constrained environments.

## [0.1.0] - 2026-03-17

### Added
- Initial standalone public packaging for `zinc-cli`.
- Human-friendly command output plus stable agent envelope.
- Wallet profile management with profile lock and atomic writes.
- PSBT create/analyze/sign/broadcast command family.
- Snapshot, account switching, and diagnostic command support.

### Changed
- `zinc-wallet-service` functionality is inlined as an internal module to keep `zinc-cli` publishable without a second public crate dependency.
