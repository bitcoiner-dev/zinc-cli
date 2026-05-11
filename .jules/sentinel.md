## 2024-03-24 - Insecure Default File Permissions
**Vulnerability:** The CLI application creates sensitive configuration files and directories (like wallets and snapshot data) using standard `fs::create_dir_all` and `fs::write` in Rust. These standard functions create files/directories using the system's default umask, which typically allows other users on the same Unix-like system to read the sensitive files.
**Learning:** This could lead to a local privilege escalation or exposure of sensitive user data if the user runs the CLI on a shared machine. Relying on default system configurations for sensitive files is unsafe.
**Prevention:** Always use `std::os::unix::fs::DirBuilderExt` and `std::os::unix::fs::OpenOptionsExt` to explicitly set file permissions (e.g., `0o700` for directories and `0o600` for files) when creating sensitive data on disk.

## 2024-03-24 - Secure File Writing Regression Prevention
**Vulnerability:** The `maybe_write_text` utility function was using `std::fs::write`, which resulted in sensitive data (like PSBT files and offers) being saved with insecure default file permissions, making them readable by other users on a shared system.
**Learning:** Even generic utility functions used for saving user-requested command outputs must use secure file permissions (`0o600`) if the data they handle (like PSBTs and offers) is sensitive.
**Prevention:** Always use `crate::paths::write_secure_file` instead of `std::fs::write` for all file writing operations that might contain sensitive material in this codebase.

## 2024-05-11 - Path Traversal in Profile/Snapshot directories
**Vulnerability:** `config.profile` was passed unvalidated into `Path::join` when calculating directories and file paths in `profile_path` and `snapshot_dir`. Because the string could contain arbitrary traversal characters, this allowed path traversal vulnerabilities.
**Learning:** In Rust, `Path::join` and similar path manipulation primitives treat `..` as a legitimate traversal step and an absolute string as a root switch. Unvalidated directory strings used for subpaths risk file system break-outs.
**Prevention:** Always validate user-provided components (like profile names or subfolder names) against a strict character allowlist (e.g. alphanumeric, `_`, `-`) to ensure they only produce flat valid file names rather than traversal paths.
