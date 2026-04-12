## 2024-03-24 - Insecure Default File Permissions
**Vulnerability:** The CLI application creates sensitive configuration files and directories (like wallets and snapshot data) using standard `fs::create_dir_all` and `fs::write` in Rust. These standard functions create files/directories using the system's default umask, which typically allows other users on the same Unix-like system to read the sensitive files.
**Learning:** This could lead to a local privilege escalation or exposure of sensitive user data if the user runs the CLI on a shared machine. Relying on default system configurations for sensitive files is unsafe.
**Prevention:** Always use `std::os::unix::fs::DirBuilderExt` and `std::os::unix::fs::OpenOptionsExt` to explicitly set file permissions (e.g., `0o700` for directories and `0o600` for files) when creating sensitive data on disk.

## 2024-03-24 - Secure File Writing Regression Prevention
**Vulnerability:** The `maybe_write_text` utility function was using `std::fs::write`, which resulted in sensitive data (like PSBT files and offers) being saved with insecure default file permissions, making them readable by other users on a shared system.
**Learning:** Even generic utility functions used for saving user-requested command outputs must use secure file permissions (`0o600`) if the data they handle (like PSBTs and offers) is sensitive.
**Prevention:** Always use `crate::paths::write_secure_file` instead of `std::fs::write` for all file writing operations that might contain sensitive material in this codebase.

## 2024-05-31 - [Path Traversal in Snapshot Command]
**Vulnerability:** Path traversal in user-provided input `name` parameter in `SnapshotAction::Save` and `SnapshotAction::Restore` allowed users to provide file paths pointing outside the snapshot directory (e.g. `../../etc/passwd`). Also identified insecure creation of directory due to standard `fs::create_dir_all`.
**Learning:** Raw user input was joined with directories to access files, potentially allowing an attacker to read/write arbitrary files, especially when not validating special characters like `..` or `/`. In addition, creating sensitive directories without proper permissions opens up to security risks.
**Prevention:** Implement strict file name allowlist validation to limit acceptable characters to alphanumerics, dashes, and underscores (e.g. using a helper like `validate_file_name`) before utilizing the input in file system operations. Also, prefer `create_secure_dir_all` over `fs::create_dir_all` to respect correct permission boundaries.
