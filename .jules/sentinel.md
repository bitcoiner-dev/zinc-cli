## 2024-03-24 - Insecure Default File Permissions
**Vulnerability:** The CLI application creates sensitive configuration files and directories (like wallets and snapshot data) using standard `fs::create_dir_all` and `fs::write` in Rust. These standard functions create files/directories using the system's default umask, which typically allows other users on the same Unix-like system to read the sensitive files.
**Learning:** This could lead to a local privilege escalation or exposure of sensitive user data if the user runs the CLI on a shared machine. Relying on default system configurations for sensitive files is unsafe.
**Prevention:** Always use `std::os::unix::fs::DirBuilderExt` and `std::os::unix::fs::OpenOptionsExt` to explicitly set file permissions (e.g., `0o700` for directories and `0o600` for files) when creating sensitive data on disk.

## 2024-05-24 - Command Injection via Configured `bitcoin_cli` Binary
**Vulnerability:** The application allowed arbitrary command execution by reading the `bitcoin_cli` command to run from a user-provided profile configuration and executing it directly with `std::process::Command::new` without validating the binary name.
**Learning:** Profile configurations or configuration files can often be manipulated by users. Trusting arbitrary paths or commands specified in these files can lead to remote code execution (RCE) or local privilege escalation if the application is run with elevated privileges.
**Prevention:** Implement a strict whitelist on the binary name allowed to be executed when the binary path is sourced from user configuration or input. Always extract the base filename and compare it against the expected executable name (e.g., `bitcoin-cli` or `bitcoin-cli.exe`).
