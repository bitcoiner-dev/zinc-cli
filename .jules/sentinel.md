## 2024-03-24 - Insecure Default File Permissions
**Vulnerability:** The CLI application creates sensitive configuration files and directories (like wallets and snapshot data) using standard `fs::create_dir_all` and `fs::write` in Rust. These standard functions create files/directories using the system's default umask, which typically allows other users on the same Unix-like system to read the sensitive files.
**Learning:** This could lead to a local privilege escalation or exposure of sensitive user data if the user runs the CLI on a shared machine. Relying on default system configurations for sensitive files is unsafe.
**Prevention:** Always use `std::os::unix::fs::DirBuilderExt` and `std::os::unix::fs::OpenOptionsExt` to explicitly set file permissions (e.g., `0o700` for directories and `0o600` for files) when creating sensitive data on disk.

## 2024-03-24 - Secure File Writing Regression Prevention
**Vulnerability:** The `maybe_write_text` utility function was using `std::fs::write`, which resulted in sensitive data (like PSBT files and offers) being saved with insecure default file permissions, making them readable by other users on a shared system.
**Learning:** Even generic utility functions used for saving user-requested command outputs must use secure file permissions (`0o600`) if the data they handle (like PSBTs and offers) is sensitive.
**Prevention:** Always use `crate::paths::write_secure_file` instead of `std::fs::write` for all file writing operations that might contain sensitive material in this codebase.

## 2024-04-16 - Prevent Path Traversal in Profile and Snapshot Processing
**Vulnerability:** The application was using unvalidated user input (`config.profile` and `name` in snapshot commands) to construct file paths for reading, writing, and creating profile and snapshot files (e.g. `snapshots/{name}.json`), resulting in a critical Path Traversal vulnerability.
**Learning:** File paths concatenated using standard path joining methods (e.g. `Path::join` and `format!`) inherently trust the supplied inputs. Without explicit validation of the filename characters, relative path components like `../` will resolve and potentially bypass intended directory sandbox constraints, exposing critical state, leading to data exposure or arbitrary file overwrites.
**Prevention:** Implement and enforce a strict path validation mechanism (e.g., `validate_file_name`) that asserts the input exclusively contains safe characters (like alphanumeric, underscores, and dashes) and rejects attempts using directories separators or empty inputs before passing the input to any file system operations.
