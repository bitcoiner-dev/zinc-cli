## 2024-03-24 - Insecure Default File Permissions
**Vulnerability:** The CLI application creates sensitive configuration files and directories (like wallets and snapshot data) using standard `fs::create_dir_all` and `fs::write` in Rust. These standard functions create files/directories using the system's default umask, which typically allows other users on the same Unix-like system to read the sensitive files.
**Learning:** This could lead to a local privilege escalation or exposure of sensitive user data if the user runs the CLI on a shared machine. Relying on default system configurations for sensitive files is unsafe.
**Prevention:** Always use `std::os::unix::fs::DirBuilderExt` and `std::os::unix::fs::OpenOptionsExt` to explicitly set file permissions (e.g., `0o700` for directories and `0o600` for files) when creating sensitive data on disk.

## 2024-03-24 - Secure File Writing Regression Prevention
**Vulnerability:** The `maybe_write_text` utility function was using `std::fs::write`, which resulted in sensitive data (like PSBT files and offers) being saved with insecure default file permissions, making them readable by other users on a shared system.
**Learning:** Even generic utility functions used for saving user-requested command outputs must use secure file permissions (`0o600`) if the data they handle (like PSBTs and offers) is sensitive.
**Prevention:** Always use `crate::paths::write_secure_file` instead of `std::fs::write` for all file writing operations that might contain sensitive material in this codebase.

## 2024-03-24 - Path Traversal in File Operations
**Vulnerability:** The snapshot command constructs file paths using `snap_dir.join(format!("{name}.json"))` directly from user-provided input without any validation. If a user provides an input like `../../../etc/passwd`, the `Path::join` method implicitly resolves the traversal, allowing them to read or write sensitive files outside the intended directory.
**Learning:** In Rust, `Path::join` will process traversal instructions like `..` and even overwrite the base path completely if given an absolute path (e.g. `/etc/passwd`). Relying on the base directory alone is insufficient when handling arbitrary user input.
**Prevention:** Always validate user-provided strings that are incorporated into file paths using an explicit allowlist (e.g., ensuring they only contain alphanumeric characters, underscores, and dashes) before passing them to path construction functions.
