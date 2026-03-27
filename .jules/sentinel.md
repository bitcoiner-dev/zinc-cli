## 2024-03-24 - Insecure Default File Permissions
**Vulnerability:** The CLI application creates sensitive configuration files and directories (like wallets and snapshot data) using standard `fs::create_dir_all` and `fs::write` in Rust. These standard functions create files/directories using the system's default umask, which typically allows other users on the same Unix-like system to read the sensitive files.
**Learning:** This could lead to a local privilege escalation or exposure of sensitive user data if the user runs the CLI on a shared machine. Relying on default system configurations for sensitive files is unsafe.
**Prevention:** Always use `std::os::unix::fs::DirBuilderExt` and `std::os::unix::fs::OpenOptionsExt` to explicitly set file permissions (e.g., `0o700` for directories and `0o600` for files) when creating sensitive data on disk.

## 2024-05-23 - Arbitrary Command Execution via Configuration Profiles
**Vulnerability:** The application executes `bitcoin-cli` using a user-provided configuration path (`profile.bitcoin_cli`) directly in `std::process::Command::new`, without verifying the actual executable name.
**Learning:** This allows an attacker who gains write access to the `.zinc-cli/profiles/*.json` configuration files to set `bitcoin_cli` to any arbitrary malicious script or binary on the system (e.g., `rm -rf /` or `/bin/sh`). When the app subsequently attempts to use `bitcoin-cli`, it executes the malicious payload instead.
**Prevention:** Always validate and restrict binary names executed via external commands, even when the path is loaded from a "trusted" local configuration file. Centralize command execution logic and enforce a strict whitelist of allowed binary filenames (e.g., exactly `bitcoin-cli` or `bitcoin-cli.exe`).
