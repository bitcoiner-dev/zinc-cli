## 2024-03-24 - Insecure Default File Permissions
**Vulnerability:** The CLI application creates sensitive configuration files and directories (like wallets and snapshot data) using standard `fs::create_dir_all` and `fs::write` in Rust. These standard functions create files/directories using the system's default umask, which typically allows other users on the same Unix-like system to read the sensitive files.
**Learning:** This could lead to a local privilege escalation or exposure of sensitive user data if the user runs the CLI on a shared machine. Relying on default system configurations for sensitive files is unsafe.
**Prevention:** Always use `std::os::unix::fs::DirBuilderExt` and `std::os::unix::fs::OpenOptionsExt` to explicitly set file permissions (e.g., `0o700` for directories and `0o600` for files) when creating sensitive data on disk.

## 2024-03-24 - Secure File Writing Regression Prevention
**Vulnerability:** The `maybe_write_text` utility function was using `std::fs::write`, which resulted in sensitive data (like PSBT files and offers) being saved with insecure default file permissions, making them readable by other users on a shared system.
**Learning:** Even generic utility functions used for saving user-requested command outputs must use secure file permissions (`0o600`) if the data they handle (like PSBTs and offers) is sensitive.
**Prevention:** Always use `crate::paths::write_secure_file` instead of `std::fs::write` for all file writing operations that might contain sensitive material in this codebase.

## 2025-02-14 - Arbitrary Binary Execution via Config Paths
**Vulnerability:** The application executed arbitrary binaries passed via the `bitcoin_cli` configuration parameter in the user profile because it only used `std::process::Command::new` without validating the path. Even though `Command::new` mitigates shell command injection, providing an arbitrary path (e.g., `/bin/sh`) could still allow arbitrary binary execution.
**Learning:** Checking for directory traversal (`..`) is insufficient. To fully prevent arbitrary binary execution when accepting an executable path from a configuration, the final filename in the path must also be strictly validated against an allowlist (e.g., ensuring it exactly matches `bitcoin-cli` or `bitcoin-cli.exe`).
**Prevention:** Always validate user-provided executable paths using strict filename matching in addition to structural validations (e.g., rejecting traversal or relative paths) before passing them to execution functions.
