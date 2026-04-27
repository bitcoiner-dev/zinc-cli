## 2024-03-24 - Insecure Default File Permissions
**Vulnerability:** The CLI application creates sensitive configuration files and directories (like wallets and snapshot data) using standard `fs::create_dir_all` and `fs::write` in Rust. These standard functions create files/directories using the system's default umask, which typically allows other users on the same Unix-like system to read the sensitive files.
**Learning:** This could lead to a local privilege escalation or exposure of sensitive user data if the user runs the CLI on a shared machine. Relying on default system configurations for sensitive files is unsafe.
**Prevention:** Always use `std::os::unix::fs::DirBuilderExt` and `std::os::unix::fs::OpenOptionsExt` to explicitly set file permissions (e.g., `0o700` for directories and `0o600` for files) when creating sensitive data on disk.

## 2024-03-24 - Secure File Writing Regression Prevention
**Vulnerability:** The `maybe_write_text` utility function was using `std::fs::write`, which resulted in sensitive data (like PSBT files and offers) being saved with insecure default file permissions, making them readable by other users on a shared system.
**Learning:** Even generic utility functions used for saving user-requested command outputs must use secure file permissions (`0o600`) if the data they handle (like PSBTs and offers) is sensitive.
**Prevention:** Always use `crate::paths::write_secure_file` instead of `std::fs::write` for all file writing operations that might contain sensitive material in this codebase.

## 2024-03-24 - Arbitrary Command Execution via Unvalidated Binary Paths
**Vulnerability:** The CLI application executed user-provided external binaries (like `bitcoin_cli` paths sourced from the configuration profile) without robust validation. Although `std::process::Command::new` mitigates shell injection (passing arguments via `execve`), an attacker could configure `bitcoin_cli` to point to a malicious absolute or relative path, or use `..` for path traversal, leading to the execution of arbitrary commands when the tool attempted to interact with the Bitcoin node.
**Learning:** External command binaries sourced from user configurations must be strictly validated. Relying solely on `Command::new`'s safety against shell injection is insufficient to prevent arbitrary command execution.
**Prevention:** Implement strict path validation (e.g., allowlist to exactly the binary name like `bitcoin-cli` or require safe absolute paths) for any executable whose path originates from user input. Reject any paths containing directory traversal characters (e.g., `..`).
