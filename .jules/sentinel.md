## 2024-03-24 - Insecure Default File Permissions
**Vulnerability:** The CLI application creates sensitive configuration files and directories (like wallets and snapshot data) using standard `fs::create_dir_all` and `fs::write` in Rust. These standard functions create files/directories using the system's default umask, which typically allows other users on the same Unix-like system to read the sensitive files.
**Learning:** This could lead to a local privilege escalation or exposure of sensitive user data if the user runs the CLI on a shared machine. Relying on default system configurations for sensitive files is unsafe.
**Prevention:** Always use `std::os::unix::fs::DirBuilderExt` and `std::os::unix::fs::OpenOptionsExt` to explicitly set file permissions (e.g., `0o700` for directories and `0o600` for files) when creating sensitive data on disk.

## 2024-03-24 - Secure File Writing Regression Prevention
**Vulnerability:** The `maybe_write_text` utility function was using `std::fs::write`, which resulted in sensitive data (like PSBT files and offers) being saved with insecure default file permissions, making them readable by other users on a shared system.
**Learning:** Even generic utility functions used for saving user-requested command outputs must use secure file permissions (`0o600`) if the data they handle (like PSBTs and offers) is sensitive.
**Prevention:** Always use `crate::paths::write_secure_file` instead of `std::fs::write` for all file writing operations that might contain sensitive material in this codebase.

## 2024-03-24 - Path Traversal via PathBuf::join with User Input
**Vulnerability:** The snapshot command was directly interpolating user-supplied strings into file names (`format!("{name}.json")`) and appending them to a base directory using `PathBuf::join()`. In Rust, if the right-hand side of `join()` is an absolute path (e.g., `/etc/passwd`) or contains parent directory traversals (`..`), the resulting path escapes the intended base directory, leading to a critical path traversal vulnerability where users could overwrite or read arbitrary files on the system.
**Learning:** Rust's `PathBuf::join()` explicitly replaces the base path if the appended string is an absolute path. It does not sanitize or restrict paths to the base directory. Relying on it without input validation is unsafe.
**Prevention:** Always strictly validate user-provided strings before using them in file paths, especially before calling `Path::join()`. Ensure the input only contains safe characters (e.g., alphanumeric, underscores, dashes) using a robust allowlist approach like `crate::utils::validate_file_name`.
