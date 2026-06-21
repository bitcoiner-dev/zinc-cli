## 2024-03-24 - Insecure Default File Permissions
**Vulnerability:** The CLI application creates sensitive configuration files and directories (like wallets and snapshot data) using standard `fs::create_dir_all` and `fs::write` in Rust. These standard functions create files/directories using the system's default umask, which typically allows other users on the same Unix-like system to read the sensitive files.
**Learning:** This could lead to a local privilege escalation or exposure of sensitive user data if the user runs the CLI on a shared machine. Relying on default system configurations for sensitive files is unsafe.
**Prevention:** Always use `std::os::unix::fs::DirBuilderExt` and `std::os::unix::fs::OpenOptionsExt` to explicitly set file permissions (e.g., `0o700` for directories and `0o600` for files) when creating sensitive data on disk.

## 2024-03-24 - Secure File Writing Regression Prevention
**Vulnerability:** The `maybe_write_text` utility function was using `std::fs::write`, which resulted in sensitive data (like PSBT files and offers) being saved with insecure default file permissions, making them readable by other users on a shared system.
**Learning:** Even generic utility functions used for saving user-requested command outputs must use secure file permissions (`0o600`) if the data they handle (like PSBTs and offers) is sensitive.
**Prevention:** Always use `crate::paths::write_secure_file` instead of `std::fs::write` for all file writing operations that might contain sensitive material in this codebase.

## 2024-03-24 - Unvalidated Path Traversal in Snapshot Commands
**Vulnerability:** The CLI constructed file paths for profiles and snapshots by passing raw user input (`name` or `config.profile`) directly to `Path::join()` (e.g., `snap_dir.join(format!("{name}.json"))`). If an attacker passed a malicious string like `../../etc/passwd`, the path resolution would traverse up and outside the intended directory, enabling critical path traversal attacks and arbitrary file reads/writes on the host OS.
**Learning:** In Rust, `Path::join` specifically replaces the base directory if the appended string starts with a root path, and blindly resolves relative directory jumps (`..`). Any user-provided identifier used to build paths must be validated strictly against an allowlist of safe characters before use.
**Prevention:** Always sanitize or strictly validate strings used in file/path construction using functions like `validate_file_name` to reject characters (like `/`, `\`, and `.`) that facilitate path traversal or traversal-related operations.
