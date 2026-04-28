## 2024-03-24 - Insecure Default File Permissions
**Vulnerability:** The CLI application creates sensitive configuration files and directories (like wallets and snapshot data) using standard `fs::create_dir_all` and `fs::write` in Rust. These standard functions create files/directories using the system's default umask, which typically allows other users on the same Unix-like system to read the sensitive files.
**Learning:** This could lead to a local privilege escalation or exposure of sensitive user data if the user runs the CLI on a shared machine. Relying on default system configurations for sensitive files is unsafe.
**Prevention:** Always use `std::os::unix::fs::DirBuilderExt` and `std::os::unix::fs::OpenOptionsExt` to explicitly set file permissions (e.g., `0o700` for directories and `0o600` for files) when creating sensitive data on disk.

## 2024-03-24 - Secure File Writing Regression Prevention
**Vulnerability:** The `maybe_write_text` utility function was using `std::fs::write`, which resulted in sensitive data (like PSBT files and offers) being saved with insecure default file permissions, making them readable by other users on a shared system.
**Learning:** Even generic utility functions used for saving user-requested command outputs must use secure file permissions (`0o600`) if the data they handle (like PSBTs and offers) is sensitive.
**Prevention:** Always use `crate::paths::write_secure_file` instead of `std::fs::write` for all file writing operations that might contain sensitive material in this codebase.

## 2024-04-28 - Arbitrary Command Execution via Unvalidated profile.bitcoin_cli
**Vulnerability:** The application was sourcing the `bitcoin_cli` executable path from the user's `Profile` configuration and passing it directly to `std::process::Command::new` without any validation. While `Command::new` mitigates shell injection (e.g., `; rm -rf /`), it still allows execution of arbitrary system binaries if a malicious or tampered user profile specifies an absolute path like `/bin/sh` or `/usr/bin/python` with arbitrary arguments.
**Learning:** Even when avoiding shell command execution, binaries explicitly invoked from user configuration must be strictly validated to ensure they are the expected executable and not an arbitrary system utility. Simply checking for an absolute path is insufficient.
**Prevention:** Always use strict validation for configurable binary paths. The `crate::utils::validate_bitcoin_cli_path` function now enforces that the path only targets exactly `bitcoin-cli` or `bitcoin-cli.exe`, rejecting any other binaries or directory traversal attempts.
