## 2024-03-24 - Insecure Default File Permissions
**Vulnerability:** The CLI application creates sensitive configuration files and directories (like wallets and snapshot data) using standard `fs::create_dir_all` and `fs::write` in Rust. These standard functions create files/directories using the system's default umask, which typically allows other users on the same Unix-like system to read the sensitive files.
**Learning:** This could lead to a local privilege escalation or exposure of sensitive user data if the user runs the CLI on a shared machine. Relying on default system configurations for sensitive files is unsafe.
**Prevention:** Always use `std::os::unix::fs::DirBuilderExt` and `std::os::unix::fs::OpenOptionsExt` to explicitly set file permissions (e.g., `0o700` for directories and `0o600` for files) when creating sensitive data on disk.

## 2024-03-24 - Secure File Writing Regression Prevention
**Vulnerability:** The `maybe_write_text` utility function was using `std::fs::write`, which resulted in sensitive data (like PSBT files and offers) being saved with insecure default file permissions, making them readable by other users on a shared system.
**Learning:** Even generic utility functions used for saving user-requested command outputs must use secure file permissions (`0o600`) if the data they handle (like PSBTs and offers) is sensitive.
**Prevention:** Always use `crate::paths::write_secure_file` instead of `std::fs::write` for all file writing operations that might contain sensitive material in this codebase.
## 2024-05-18 - Path Traversal via PathBuf::join with Unvalidated Input
**Vulnerability:** Constructing file paths dynamically using `PathBuf::join(format!("{name}.json"))` with unvalidated user input (`name` from CLI arguments) allows an attacker to perform path traversal using `../` (e.g., `--name ../../../etc/passwd`). Furthermore, standard `fs::create_dir_all` might fall back to insecure default permissions depending on the caller.
**Learning:** The `join` method on `Path` or `PathBuf` does not validate or sanitize components. If an attacker controls the input and provides absolute paths or traversal segments (`../`), it resolves outside the intended directory.
**Prevention:** Always validate user-provided file names before appending them to paths. Use strict allowlists (`[a-zA-Z0-9_-]+`) for custom names. Remove redundant and standard directory creation functions if secure wrappers (e.g., `create_secure_dir_all`) are already implemented and called upstream.
