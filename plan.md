1. **Edit `src/utils.rs` to define `validate_file_name` function.**
   - Add `pub fn validate_file_name(name: &str) -> Result<(), AppError>` that checks if the name is not empty and only contains alphanumeric characters, underscores, or dashes.
   - Return `AppError::Invalid("invalid file name: must only contain alphanumeric characters, underscores, and dashes".to_string())` on failure.

2. **Edit `src/commands/snapshot.rs` to validate `name`.**
   - In `SnapshotAction::Save`, call `crate::utils::validate_file_name(name)?;` before using `name` in `snap_dir.join()`.
   - In `SnapshotAction::Restore`, call `crate::utils::validate_file_name(name)?;` before using `name` in `snap_dir.join()`.

3. **Edit `src/paths.rs` to validate `config.profile`.**
   - In `profile_path`, call `crate::utils::validate_file_name(config.profile)?;` before using it in `profiles.join()`.
   - In `snapshot_dir`, call `crate::utils::validate_file_name(config.profile)?;` before using it in `root.join("snapshots").join()`.

4. **Add entry to `.jules/sentinel.md` journal.**
   - Append an entry for the CRITICAL path traversal vulnerability.

5. **Verify changes using `cargo check` and `cargo test --bin zinc-cli`.**
   - Run `cargo check` to ensure syntax is valid.
   - Run `cargo test --bin zinc-cli` as the final explicit action to test the code.

6. **Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.**
   - Call `pre_commit_instructions` and follow steps.

7. **Submit the PR.**
   - Create PR "🛡️ Sentinel: [CRITICAL] Fix path traversal vulnerability in profile and snapshot paths".
