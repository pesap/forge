use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::blueprint::python_library::{
    clean_optional_files, config_from_pyproject, render_managed_files,
};
use crate::cli::UpgradeArgs;

pub fn run(args: UpgradeArgs) -> Result<()> {
    let root = args.path.canonicalize().unwrap_or(args.path);
    let pyproject_path = root.join("pyproject.toml");
    let pyproject = fs::read_to_string(&pyproject_path)
        .with_context(|| format!("failed to read {}", pyproject_path.display()))?;

    let config = config_from_pyproject(&pyproject)?;
    config.validate()?;

    let managed_files = render_managed_files(&config);
    for (relative_path, content) in managed_files {
        let full_path = root.join(relative_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&full_path, content)
            .with_context(|| format!("failed to write {}", full_path.display()))?;
    }

    clean_optional_files(Path::new(&root), &config)?;

    println!(
        "Upgraded managed infrastructure files for {}",
        root.display()
    );
    Ok(())
}
