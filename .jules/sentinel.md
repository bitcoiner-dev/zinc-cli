## 2024-03-24 - Insecure Default File Permissions
**Vulnerability:** The CLI application creates sensitive configuration files and directories (like wallets and snapshot data) using standard `fs::create_dir_all` and `fs::write` in Rust. These standard functions create files/directories using the system's default umask, which typically allows other users on the same Unix-like system to read the sensitive files.
**Learning:** This could lead to a local privilege escalation or exposure of sensitive user data if the user runs the CLI on a shared machine. Relying on default system configurations for sensitive files is unsafe.
**Prevention:** Always use `std::os::unix::fs::DirBuilderExt` and `std::os::unix::fs::OpenOptionsExt` to explicitly set file permissions (e.g., `0o700` for directories and `0o600` for files) when creating sensitive data on disk.

## 2024-03-24 - Secure File Writing Regression Prevention
**Vulnerability:** The `maybe_write_text` utility function was using `std::fs::write`, which resulted in sensitive data (like PSBT files and offers) being saved with insecure default file permissions, making them readable by other users on a shared system.
**Learning:** Even generic utility functions used for saving user-requested command outputs must use secure file permissions (`0o600`) if the data they handle (like PSBTs and offers) is sensitive.
**Prevention:** Always use `crate::paths::write_secure_file` instead of `std::fs::write` for all file writing operations that might contain sensitive material in this codebase.

## 2024-05-23 - Path Traversal in Snapshot Commands
**Vulnerability:** User-provided snapshot names in commands like `zinc snapshot save <name>` or `zinc snapshot restore <name>` were directly passed to `Path::join()` (e.g., `snap_dir.join(format!("{name}.json"))`) without any validation. Because `Path::join` processes `../` strings and absolute paths, a malicious or accidental argument could navigate outside the intended snapshots directory, allowing unauthorized reading or overwriting of arbitrary files on the filesystem.
**Learning:** Functions that combine user inputs with local file paths (like `Path::join` or `PathBuf::join`) are inherently vulnerable to path traversal. In Rust, standard path joining resolves directory traversal strings (`..`) natively and even replaces the entire path if the appended segment is absolute (e.g. starting with `/`).
**Prevention:** Always validate user-provided strings before using them in file paths, especially when utilizing `Path::join()`. Utilize the newly introduced `crate::utils::validate_file_name` function to strictly allowlist valid filename characters (alphanumeric, underscores, dashes) and reject empty strings or path traversal sequences.
