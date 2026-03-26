## 2024-03-26 - Centralize and secure bitcoin-cli execution

**Vulnerability:** `run_bitcoin_cli` is defined in two places (`src/utils.rs` and `src/wallet_service.rs`) and executes the `bitcoin_cli` command defined in a user-provided profile without strict validation, potentially allowing arbitrary command execution if the profile is tampered with or maliciously constructed.
**Learning:** External command execution based on configurable profile values must be strictly validated against an allowlist (e.g., only "bitcoin-cli" or "bitcoin-cli.exe") to prevent arbitrary command injection. Duplicated security-critical functions make auditing harder.
**Prevention:** Centralize external process execution and enforce strict validation on executable names derived from configurations.
