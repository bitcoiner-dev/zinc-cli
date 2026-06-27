## 2024-03-24 - Insecure Default File Permissions
**Vulnerability:** The CLI application creates sensitive configuration files and directories (like wallets and snapshot data) using standard `fs::create_dir_all` and `fs::write` in Rust. These standard functions create files/directories using the system's default umask, which typically allows other users on the same Unix-like system to read the sensitive files.
**Learning:** This could lead to a local privilege escalation or exposure of sensitive user data if the user runs the CLI on a shared machine. Relying on default system configurations for sensitive files is unsafe.
**Prevention:** Always use `std::os::unix::fs::DirBuilderExt` and `std::os::unix::fs::OpenOptionsExt` to explicitly set file permissions (e.g., `0o700` for directories and `0o600` for files) when creating sensitive data on disk.

## 2024-03-24 - Secure File Writing Regression Prevention
**Vulnerability:** The `maybe_write_text` utility function was using `std::fs::write`, which resulted in sensitive data (like PSBT files and offers) being saved with insecure default file permissions, making them readable by other users on a shared system.
**Learning:** Even generic utility functions used for saving user-requested command outputs must use secure file permissions (`0o600`) if the data they handle (like PSBTs and offers) is sensitive.
**Prevention:** Always use `crate::paths::write_secure_file` instead of `std::fs::write` for all file writing operations that might contain sensitive material in this codebase.

## 2024-05-18 - [CRITICAL] Path Traversal in Path::join
**Vulnerability:** Path traversal vulnerability in `Path::join` when appending an unvalidated string (e.g., in `src/commands/snapshot.rs` via user-provided snapshot names) which allows traversing upwards using `..` or replacing the base directory with an absolute path.
**Learning:** `Path::join` in Rust natively traverses when given absolute paths or `..`. Passing unchecked string input to it directly undermines sandbox or directory confinement.
**Prevention:** Implement strict string validation (e.g., allowlisting only alphanumeric characters, underscores, and dashes via `crate::utils::validate_file_name`) before passing strings into `Path::join`.
