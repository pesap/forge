pub mod blueprints;
pub mod completions;
pub mod components;
pub mod diff;
pub mod doctor;
pub mod init;
mod managed;
pub mod new;
pub mod self_update;
pub mod sync;

use anyhow::Result;

use crate::errors::{ErrorCode, coded_error};

/// Validate that --diff is paired with a non-writing mode.
pub fn validate_diff_mode(diff: bool, dry_run: bool, check: bool) -> Result<()> {
    if !diff {
        return Ok(());
    }
    if dry_run || check {
        return Ok(());
    }
    Err(coded_error(
        ErrorCode::Input,
        "--diff requires --dry-run or --check",
    ))
}
