## 2024-03-24 - Insecure Default File Permissions
**Vulnerability:** The CLI application creates sensitive configuration files and directories (like wallets and snapshot data) using standard `fs::create_dir_all` and `fs::write` in Rust. These standard functions create files/directories using the system's default umask, which typically allows other users on the same Unix-like system to read the sensitive files.
**Learning:** This could lead to a local privilege escalation or exposure of sensitive user data if the user runs the CLI on a shared machine. Relying on default system configurations for sensitive files is unsafe.
**Prevention:** Always use `std::os::unix::fs::DirBuilderExt` and `std::os::unix::fs::OpenOptionsExt` to explicitly set file permissions (e.g., `0o700` for directories and `0o600` for files) when creating sensitive data on disk.

## 2024-03-24 - Secure File Writing Regression Prevention
**Vulnerability:** The `maybe_write_text` utility function was using `std::fs::write`, which resulted in sensitive data (like PSBT files and offers) being saved with insecure default file permissions, making them readable by other users on a shared system.
**Learning:** Even generic utility functions used for saving user-requested command outputs must use secure file permissions (`0o600`) if the data they handle (like PSBTs and offers) is sensitive.
**Prevention:** Always use `crate::paths::write_secure_file` instead of `std::fs::write` for all file writing operations that might contain sensitive material in this codebase.

## 2024-05-10 - Arbitrary Command Execution via Unvalidated Binary Path
**Vulnerability:** The CLI application allowed users to specify arbitrary paths for external binaries (like `bitcoin-cli`) in their user profile configurations. These paths were executed without any validation using `std::process::Command::new`, making the application vulnerable to executing arbitrary system commands or malicious binaries under the application's context (e.g., executing `/bin/sh` or traverse directories with `../`).
**Learning:** Even if the command argument vector itself is not subject to shell injection because `Command::new` uses `execve` directly, allowing an unvalidated executable path to be run can lead to complete host takeover.
**Prevention:** All executable paths sourced from user-supplied configurations must be strictly validated. Enforce that the binary path only references allowed executables (like exactly `bitcoin-cli` or `bitcoin-cli.exe`) and explicitly block directory traversals (e.g., `..`).
