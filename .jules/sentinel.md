## 2024-03-24 - Insecure Default File Permissions
**Vulnerability:** The CLI application creates sensitive configuration files and directories (like wallets and snapshot data) using standard `fs::create_dir_all` and `fs::write` in Rust. These standard functions create files/directories using the system's default umask, which typically allows other users on the same Unix-like system to read the sensitive files.
**Learning:** This could lead to a local privilege escalation or exposure of sensitive user data if the user runs the CLI on a shared machine. Relying on default system configurations for sensitive files is unsafe.
**Prevention:** Always use `std::os::unix::fs::DirBuilderExt` and `std::os::unix::fs::OpenOptionsExt` to explicitly set file permissions (e.g., `0o700` for directories and `0o600` for files) when creating sensitive data on disk.

## 2024-03-24 - Secure File Writing Regression Prevention
**Vulnerability:** The `maybe_write_text` utility function was using `std::fs::write`, which resulted in sensitive data (like PSBT files and offers) being saved with insecure default file permissions, making them readable by other users on a shared system.
**Learning:** Even generic utility functions used for saving user-requested command outputs must use secure file permissions (`0o600`) if the data they handle (like PSBTs and offers) is sensitive.
**Prevention:** Always use `crate::paths::write_secure_file` instead of `std::fs::write` for all file writing operations that might contain sensitive material in this codebase.

## 2024-03-24 - Insecure Directory Creation Regression Prevention
**Vulnerability:** The standard `std::fs::create_dir_all` was used to create sensitive user profile directories, temporary directories handling sensitive pulse session data, and snapshot directories in `src/commands/intent.rs`, `src/commands/pulse.rs`, and `src/commands/snapshot.rs`. Standard `fs::create_dir_all` relies on the system's default umask and may create directories that are readable by other users.
**Learning:** Even internal utility and test functions must use secure directory creation wrappers to avoid regressions and ensure sensitive directories containing wallet data or intents are properly secured against local access.
**Prevention:** Always use `crate::paths::create_secure_dir_all` instead of `std::fs::create_dir_all` when creating directories that might contain sensitive material in this codebase.
