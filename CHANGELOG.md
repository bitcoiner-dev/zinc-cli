# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

- No changes yet.

## [0.1.0] - 2026-03-17

### Added
- Initial standalone public packaging for `zinc-cli`.
- Human-friendly command output plus stable `--json` agent envelope.
- Wallet profile management with profile lock and atomic writes.
- PSBT create/analyze/sign/broadcast command family.
- Snapshot, account switching, and diagnostic command support.

### Changed
- `zinc-wallet-service` functionality is inlined as an internal module to keep `zinc-cli` publishable without a second public crate dependency.
