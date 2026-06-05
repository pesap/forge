pub mod blueprints;
pub mod completions;
pub mod components;
mod dependency_groups;
pub mod diff;
pub mod doctor;
pub mod init;
pub mod new;
mod pyproject_sections;
pub mod self_update;
pub mod sync;

use anyhow::Result;

use crate::errors::{ErrorCode, coded_error};

/// Validate --diff flag: it requires --dry-run (or --check for update).
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
