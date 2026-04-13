## 2024-03-24 - Insecure Default File Permissions
**Vulnerability:** The CLI application creates sensitive configuration files and directories (like wallets and snapshot data) using standard `fs::create_dir_all` and `fs::write` in Rust. These standard functions create files/directories using the system's default umask, which typically allows other users on the same Unix-like system to read the sensitive files.
**Learning:** This could lead to a local privilege escalation or exposure of sensitive user data if the user runs the CLI on a shared machine. Relying on default system configurations for sensitive files is unsafe.
**Prevention:** Always use `std::os::unix::fs::DirBuilderExt` and `std::os::unix::fs::OpenOptionsExt` to explicitly set file permissions (e.g., `0o700` for directories and `0o600` for files) when creating sensitive data on disk.

## 2024-03-24 - Secure File Writing Regression Prevention
**Vulnerability:** The `maybe_write_text` utility function was using `std::fs::write`, which resulted in sensitive data (like PSBT files and offers) being saved with insecure default file permissions, making them readable by other users on a shared system.
**Learning:** Even generic utility functions used for saving user-requested command outputs must use secure file permissions (`0o600`) if the data they handle (like PSBTs and offers) is sensitive.
**Prevention:** Always use `crate::paths::write_secure_file` instead of `std::fs::write` for all file writing operations that might contain sensitive material in this codebase.

## 2024-04-13 - Path Traversal on Profile and Snapshot Names
**Vulnerability:** The application was constructing file paths for profiles (`profiles/{profile_name}.json`) and snapshots (`snapshots/{profile_name}/{snapshot_name}.json`) without sanitizing or validating the `profile_name` or `snapshot_name` variables. This could allow an attacker to read, write, or overwrite arbitrary files on the system using path traversal payloads like `../../`.
**Learning:** Any user-provided or configurable string that is used to construct a file path must be strictly validated to prevent path traversal, even if the application otherwise handles the actual file read/write operations securely.
**Prevention:** Use a dedicated validation function (e.g., `crate::utils::validate_file_name`) to enforce a strict allowlist of characters (such as alphanumeric, underscores, and dashes) on file and directory names before using them in `Path::join` operations.
