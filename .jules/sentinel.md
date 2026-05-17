## 2024-03-24 - Insecure Default File Permissions
**Vulnerability:** The CLI application creates sensitive configuration files and directories (like wallets and snapshot data) using standard `fs::create_dir_all` and `fs::write` in Rust. These standard functions create files/directories using the system's default umask, which typically allows other users on the same Unix-like system to read the sensitive files.
**Learning:** This could lead to a local privilege escalation or exposure of sensitive user data if the user runs the CLI on a shared machine. Relying on default system configurations for sensitive files is unsafe.
**Prevention:** Always use `std::os::unix::fs::DirBuilderExt` and `std::os::unix::fs::OpenOptionsExt` to explicitly set file permissions (e.g., `0o700` for directories and `0o600` for files) when creating sensitive data on disk.

## 2024-03-24 - Secure File Writing Regression Prevention
**Vulnerability:** The `maybe_write_text` utility function was using `std::fs::write`, which resulted in sensitive data (like PSBT files and offers) being saved with insecure default file permissions, making them readable by other users on a shared system.
**Learning:** Even generic utility functions used for saving user-requested command outputs must use secure file permissions (`0o600`) if the data they handle (like PSBTs and offers) is sensitive.
**Prevention:** Always use `crate::paths::write_secure_file` instead of `std::fs::write` for all file writing operations that might contain sensitive material in this codebase.

## 2024-05-17 - Path Traversal in File Paths
**Vulnerability:** User-provided strings (such as snapshot names and profile names) were used directly in `Path::join` to construct file paths for configuration and data directories.
**Learning:** In Rust, `Path::join` replaces the base path entirely if the appended string is an absolute path (e.g., `/etc/passwd`), and it resolves `..` components. Passing unvalidated user input to `Path::join` enables path traversal and arbitrary file read/write vulnerabilities.
**Prevention:** Always validate user-provided strings used in file paths using a strict allowlist. Use a utility function like `crate::utils::validate_file_name` to ensure the input only contains safe characters (e.g., alphanumeric, underscores, dashes) before passing it to `Path::join`.
