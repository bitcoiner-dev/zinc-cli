## 2024-03-24 - Insecure Default File Permissions
**Vulnerability:** The CLI application creates sensitive configuration files and directories (like wallets and snapshot data) using standard `fs::create_dir_all` and `fs::write` in Rust. These standard functions create files/directories using the system's default umask, which typically allows other users on the same Unix-like system to read the sensitive files.
**Learning:** This could lead to a local privilege escalation or exposure of sensitive user data if the user runs the CLI on a shared machine. Relying on default system configurations for sensitive files is unsafe.
**Prevention:** Always use `std::os::unix::fs::DirBuilderExt` and `std::os::unix::fs::OpenOptionsExt` to explicitly set file permissions (e.g., `0o700` for directories and `0o600` for files) when creating sensitive data on disk.

## 2024-03-24 - Secure File Writing Regression Prevention
**Vulnerability:** The `maybe_write_text` utility function was using `std::fs::write`, which resulted in sensitive data (like PSBT files and offers) being saved with insecure default file permissions, making them readable by other users on a shared system.
**Learning:** Even generic utility functions used for saving user-requested command outputs must use secure file permissions (`0o600`) if the data they handle (like PSBTs and offers) is sensitive.
**Prevention:** Always use `crate::paths::write_secure_file` instead of `std::fs::write` for all file writing operations that might contain sensitive material in this codebase.

## 2024-06-03 - Path Traversal Vulnerability
**Vulnerability:** A path traversal vulnerability existed in `src/commands/snapshot.rs` where user input (the `name` argument) was directly joined with the snapshot directory path using `PathBuf::join`, allowing arbitrary files (e.g., `/etc/passwd`) to be overwritten or read.
**Learning:** `PathBuf::join` directly handles absolute paths by replacing the base directory, meaning if `name` is an absolute path, it ignores the intended base directory.
**Prevention:** Always validate and sanitize user-provided filenames before using them in file path construction, especially with `join`. Use a strict validation function like `validate_file_name` to ensure only safe alphanumeric characters are accepted.
