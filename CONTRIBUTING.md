# Contributing

Thanks for helping improve `zinc-cli`.

## Development Setup

1. Install stable Rust.
2. From the workspace root (`wallet/zinc-core`), run:

```bash
cargo fmt --all -- --check
cargo test -p zinc-core
cargo clippy -p zinc-core --all-targets --all-features -- -D warnings
cargo doc -p zinc-core --no-deps
```

## Pull Requests

- Keep changes focused and well-scoped.
- Add or update tests for behavior changes.
- Update docs/examples if public APIs or user workflows change.
- Document user-visible changes in `CHANGELOG.md`.

## Release Checklist (`zinc-cli` -> `zinc-core`)

1. Publish `zinc-core` to crates.io first.
2. Update `Cargo.toml` to an exact `zinc-core` pin:
   - `zinc-core = { version = "=X.Y.Z" }`
3. Push a `zinc-cli` release tag.
4. Ensure tag CI passes:
   - exact pin validation succeeds,
   - `cargo package` verification succeeds against released crates.io dependencies.

## Security-Sensitive Code

Changes related to key handling, signing, or transaction safety should include:

- threat/abuse scenario coverage,
- explicit test coverage, and
- careful review of failure paths.
