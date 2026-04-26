## 2024-03-24 - Insecure Default File Permissions
**Vulnerability:** The CLI application creates sensitive configuration files and directories (like wallets and snapshot data) using standard `fs::create_dir_all` and `fs::write` in Rust. These standard functions create files/directories using the system's default umask, which typically allows other users on the same Unix-like system to read the sensitive files.
**Learning:** This could lead to a local privilege escalation or exposure of sensitive user data if the user runs the CLI on a shared machine. Relying on default system configurations for sensitive files is unsafe.
**Prevention:** Always use `std::os::unix::fs::DirBuilderExt` and `std::os::unix::fs::OpenOptionsExt` to explicitly set file permissions (e.g., `0o700` for directories and `0o600` for files) when creating sensitive data on disk.

## 2024-03-24 - Secure File Writing Regression Prevention
**Vulnerability:** The `maybe_write_text` utility function was using `std::fs::write`, which resulted in sensitive data (like PSBT files and offers) being saved with insecure default file permissions, making them readable by other users on a shared system.
**Learning:** Even generic utility functions used for saving user-requested command outputs must use secure file permissions (`0o600`) if the data they handle (like PSBTs and offers) is sensitive.
**Prevention:** Always use `crate::paths::write_secure_file` instead of `std::fs::write` for all file writing operations that might contain sensitive material in this codebase.

## 2024-05-18 - Prevent Path Traversal via Unvalidated User Input
**Vulnerability:** Path traversal was possible in the snapshot command because user-provided file names were directly appended to directory paths using `.join()` without validating that they didn't contain directory traversal sequences like `..` or `/`.
**Learning:** In Rust, `Path::join()` naturally traverses upwards when given `..` and explicitly replaces the base directory if given an absolute path (e.g., starting with `/`). Always validate user-provided strings against a strict allowlist (e.g., alphanumeric, underscores, dashes) before using them in file paths to prevent critical path traversal attacks.
**Prevention:** Implement and enforce a strict allowlist character validation function (e.g., `validate_file_name`) for any user input that is incorporated into file paths.
