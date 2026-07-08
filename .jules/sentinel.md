## 2024-03-24 - Insecure Default File Permissions
**Vulnerability:** The CLI application creates sensitive configuration files and directories (like wallets and snapshot data) using standard `fs::create_dir_all` and `fs::write` in Rust. These standard functions create files/directories using the system's default umask, which typically allows other users on the same Unix-like system to read the sensitive files.
**Learning:** This could lead to a local privilege escalation or exposure of sensitive user data if the user runs the CLI on a shared machine. Relying on default system configurations for sensitive files is unsafe.
**Prevention:** Always use `std::os::unix::fs::DirBuilderExt` and `std::os::unix::fs::OpenOptionsExt` to explicitly set file permissions (e.g., `0o700` for directories and `0o600` for files) when creating sensitive data on disk.

## 2024-03-24 - Secure File Writing Regression Prevention
**Vulnerability:** The `maybe_write_text` utility function was using `std::fs::write`, which resulted in sensitive data (like PSBT files and offers) being saved with insecure default file permissions, making them readable by other users on a shared system.
**Learning:** Even generic utility functions used for saving user-requested command outputs must use secure file permissions (`0o600`) if the data they handle (like PSBTs and offers) is sensitive.
**Prevention:** Always use `crate::paths::write_secure_file` instead of `std::fs::write` for all file writing operations that might contain sensitive material in this codebase.

---

# Resolved / out-of-scope findings — DO NOT re-report

The items below have been triaged and either fixed or determined to be by design.
Re-opening PRs for them creates duplicate noise. Verify the cited code before
filing any new report on these topics.

## 2026-06-29 - Path Traversal in Snapshot Names — RESOLVED
**Status:** Fixed. Do not re-report.
**Vulnerability:** `snapshot save` / `snapshot restore` joined the user-supplied `--name` onto the snapshot directory via `Path::join` without validation, so a name containing `/`, `..`, or an absolute path could read or write outside the snapshot directory.
**Resolution:** `crate::utils::validate_file_name()` rejects empty names, path separators (`/`, `\`), `..`, NUL, absolute paths, and leading dots. It is called at the top of both the `Save` and `Restore` arms in `src/commands/snapshot.rs`. Covered by `validate_file_name_*` tests in `src/utils.rs`.
**Prevention:** Any new code that turns user input into a filename component must call `validate_file_name()` before `Path::join`.

## 2026-06-29 - Insecure File/Directory Permissions — ALREADY RESOLVED
**Status:** Already handled across the codebase. Do not re-report.
**Where it is handled:** `src/paths.rs` creates directories with `0o700` (`create_secure_dir_all`) and writes files with `0o600` (`write_secure_file`); `write_bytes_atomic` and `maybe_write_text` both route through these. `src/lock.rs` opens the lock file with `mode(0o600)`. The lone `fs::create_dir_all` in `src/commands/snapshot.rs` is a harmless no-op: `snapshot_dir()` has already created that directory securely with `0o700` on the line above.
**Prevention:** Use `crate::paths::create_secure_dir_all` / `write_secure_file` (never bare `fs::create_dir_all` / `fs::write`) for any new sensitive on-disk data.

## 2026-06-29 - `profile.bitcoin_cli` Execution — BY DESIGN, NOT A VULNERABILITY
**Status:** Working as intended. Do not report as a vulnerability.
**Analysis:** `run_bitcoin_cli` executes `std::process::Command::new(&profile.bitcoin_cli).args(...)`. There is no shell, so there is no `sh -c` / command-injection surface. The binary path and its arguments come from the user's own profile config on their own machine — the same trust level as the user invoking the CLI. A configurable external-binary path (custom `bitcoin-cli` location/flags) is a required feature, not attacker-controllable input.
**Prevention:** Keep external-process invocation argv-based (`Command::args`), never a shell string. No validation of the user-owned binary path is required.

## 2024-03-24 - Redundant and Insecure Directory Creation
**Vulnerability:** In `src/commands/snapshot.rs`, `fs::create_dir_all(&snap_dir)` is called using standard library functions, which rely on the default umask (insecure defaults for sensitive data). However, this call is actually completely redundant and harmless because the line right above it, `snapshot_dir(cli)?`, already securely creates the same directory with `0o700` permissions.
**Learning:** Redundant standard library file system operations for sensitive directories can cause confusion during security audits, leading to false positives about insecure default permissions, even if the directory was previously created securely.
**Prevention:** Avoid redundant `fs::create_dir_all` calls when the underlying path resolution function (like `snapshot_dir`) already securely handles directory initialization.
