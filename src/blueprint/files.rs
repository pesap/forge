use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GeneratedFile {
    Text(String),
    Symlink { target: PathBuf },
}

impl GeneratedFile {
    pub fn text(content: impl Into<String>) -> Self {
        Self::Text(content.into())
    }

    pub fn symlink(target: impl Into<PathBuf>) -> Self {
        Self::Symlink {
            target: target.into(),
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(content) => Some(content),
            Self::Symlink { .. } => None,
        }
    }

    pub fn symlink_target(&self) -> Option<&Path> {
        match self {
            Self::Text(_) => None,
            Self::Symlink { target } => Some(target),
        }
    }
}

pub type GeneratedFiles = BTreeMap<PathBuf, GeneratedFile>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManagedFileAction {
    Create(PathBuf),
    Update(PathBuf),
    MetadataAppend(PathBuf),
    Keep(PathBuf),
    PreserveUserFile(PathBuf),
    PreserveSemanticEquivalent(PathBuf),
    Relocate {
        from: PathBuf,
        to: PathBuf,
    },
    Relink(PathBuf),
    Remove(PathBuf),
    Conflict {
        path: PathBuf,
        reason: ManagedFileConflict,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagedFileConflict {
    Directory,
    ExistingFile,
    NotUtf8,
    Unreadable,
    ParentNotDirectory,
    UnsafePath,
}

impl ManagedFileAction {
    pub fn path(&self) -> &Path {
        match self {
            Self::Create(path)
            | Self::Update(path)
            | Self::MetadataAppend(path)
            | Self::Keep(path)
            | Self::PreserveUserFile(path)
            | Self::PreserveSemanticEquivalent(path)
            | Self::Remove(path)
            | Self::Relink(path) => path,
            Self::Relocate { to, .. } => to,
            Self::Conflict { path, .. } => path,
        }
    }

    pub fn source_path(&self) -> Option<&Path> {
        match self {
            Self::Relocate { from, .. } => Some(from),
            _ => None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Create(_) => "create",
            Self::Update(_) | Self::MetadataAppend(_) => "update",
            Self::Keep(_) => "keep",
            Self::PreserveUserFile(_) | Self::PreserveSemanticEquivalent(_) => "preserve",
            Self::Relocate { .. } => "relocate",
            Self::Relink(_) => "relink",
            Self::Remove(_) => "remove",
            Self::Conflict { .. } => "conflict",
        }
    }

    pub fn changes_filesystem(&self) -> bool {
        !matches!(
            self,
            Self::Keep(_)
                | Self::PreserveUserFile(_)
                | Self::PreserveSemanticEquivalent(_)
                | Self::Conflict { .. }
        )
    }

    pub fn blocks_update(&self) -> bool {
        matches!(self, Self::Conflict { .. })
    }

    pub fn reason(&self) -> Option<&'static str> {
        match self {
            Self::Create(_) => Some("new_managed_file"),
            Self::MetadataAppend(_) => Some("metadata_append"),
            Self::PreserveUserFile(_) => Some("existing_user_file_preserved; takeover_required"),
            Self::PreserveSemanticEquivalent(_) => {
                Some("existing_semantic_equivalent_preserved; takeover_required")
            }
            Self::Relocate { .. } => Some("takeover_relocated"),
            Self::Conflict { reason, .. } => Some(reason.as_str()),
            _ => None,
        }
    }

    pub fn reason_code(&self) -> Option<&'static str> {
        match self {
            Self::Create(_) => Some("new_managed_file"),
            Self::MetadataAppend(_) => Some("metadata_append"),
            Self::PreserveUserFile(_) => Some("existing_user_file_preserved"),
            Self::PreserveSemanticEquivalent(_) => Some("existing_semantic_equivalent_preserved"),
            Self::Relocate { .. } => Some("takeover_relocated"),
            Self::Conflict { reason, .. } => Some(reason.code()),
            _ => None,
        }
    }
}

impl ManagedFileConflict {
    pub fn code(self) -> &'static str {
        match self {
            Self::Directory => "directory",
            Self::ExistingFile => "existing_file",
            Self::NotUtf8 => "not_utf8",
            Self::Unreadable => "unreadable",
            Self::ParentNotDirectory => "parent_not_directory",
            Self::UnsafePath => "unsafe_path",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Directory => "managed path is a directory",
            Self::ExistingFile => "existing file would be overwritten",
            Self::NotUtf8 => "managed text file is not UTF-8",
            Self::Unreadable => "managed text file cannot be read",
            Self::ParentNotDirectory => "managed parent path is not a directory",
            Self::UnsafePath => "managed path must stay inside the repository",
        }
    }
}

pub fn plan_generated_files(root: &Path, files: &GeneratedFiles) -> Vec<ManagedFileAction> {
    files
        .iter()
        .map(|(relative_path, generated_file)| {
            let Ok(full_path) = managed_file_path(root, relative_path) else {
                return ManagedFileAction::Conflict {
                    path: relative_path.clone(),
                    reason: ManagedFileConflict::UnsafePath,
                };
            };
            if let Some(action) = parent_directory_conflict(root, relative_path) {
                return action;
            }
            if symlink_target_is_unsafe(generated_file) {
                return ManagedFileAction::Conflict {
                    path: relative_path.clone(),
                    reason: ManagedFileConflict::UnsafePath,
                };
            }
            plan_generated_file(relative_path, &full_path, generated_file)
        })
        .collect()
}

pub fn managed_file_path(root: &Path, relative_path: &Path) -> Result<PathBuf> {
    ensure_safe_managed_path(relative_path)?;
    Ok(root.join(relative_path))
}

fn ensure_safe_managed_path(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty() {
        bail!("managed path must include a file name");
    }

    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => bail!(
                "managed path must stay inside the repository: {}",
                path.display()
            ),
        }
    }

    Ok(())
}

fn symlink_target_is_unsafe(generated_file: &GeneratedFile) -> bool {
    match generated_file {
        GeneratedFile::Text(_) => false,
        GeneratedFile::Symlink { target } => ensure_safe_managed_path(target).is_err(),
    }
}

fn parent_directory_conflict(root: &Path, relative_path: &Path) -> Option<ManagedFileAction> {
    let mut current = relative_path.parent()?;
    while !current.as_os_str().is_empty() {
        let full_path = root.join(current);
        match full_path.symlink_metadata() {
            Ok(metadata) if !metadata.is_dir() => {
                return Some(ManagedFileAction::Conflict {
                    path: relative_path.into(),
                    reason: ManagedFileConflict::ParentNotDirectory,
                });
            }
            Ok(_) => {}
            Err(_) => {}
        }
        current = current.parent()?;
    }

    None
}

fn plan_generated_file(
    relative_path: &Path,
    full_path: &Path,
    generated_file: &GeneratedFile,
) -> ManagedFileAction {
    match generated_file {
        GeneratedFile::Text(content) => match fs::read_to_string(full_path) {
            Ok(existing) if existing == *content => ManagedFileAction::Keep(relative_path.into()),
            Ok(_) => ManagedFileAction::Update(relative_path.into()),
            Err(error) if error.kind() == std::io::ErrorKind::IsADirectory => {
                ManagedFileAction::Conflict {
                    path: relative_path.into(),
                    reason: ManagedFileConflict::Directory,
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::InvalidData => {
                ManagedFileAction::Conflict {
                    path: relative_path.into(),
                    reason: ManagedFileConflict::NotUtf8,
                }
            }
            Err(_) if full_path.symlink_metadata().is_ok() => ManagedFileAction::Conflict {
                path: relative_path.into(),
                reason: ManagedFileConflict::Unreadable,
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                ManagedFileAction::Create(relative_path.into())
            }
            Err(_) => ManagedFileAction::Create(relative_path.into()),
        },
        GeneratedFile::Symlink { target } => match fs::read_link(full_path) {
            Ok(existing) if existing == *target => ManagedFileAction::Keep(relative_path.into()),
            Ok(_) => ManagedFileAction::Relink(relative_path.into()),
            Err(_) if full_path.is_dir() => ManagedFileAction::Conflict {
                path: relative_path.into(),
                reason: ManagedFileConflict::Directory,
            },
            Err(_) if full_path.exists() || full_path.symlink_metadata().is_ok() => {
                if managed_link_fallback_matches_target(full_path, target) {
                    ManagedFileAction::Keep(relative_path.into())
                } else {
                    ManagedFileAction::Relink(relative_path.into())
                }
            }
            Err(_) => ManagedFileAction::Create(relative_path.into()),
        },
    }
}

pub fn write_generated_file(path: &Path, generated_file: &GeneratedFile) -> Result<()> {
    match generated_file {
        GeneratedFile::Text(content) => write_text_file_atomically(path, content),
        GeneratedFile::Symlink { target } => replace_symlink(path, target),
    }
}

pub fn count_changes(actions: &[ManagedFileAction]) -> usize {
    actions
        .iter()
        .filter(|action| action.changes_filesystem())
        .count()
}

pub fn count_conflicts(actions: &[ManagedFileAction]) -> usize {
    actions
        .iter()
        .filter(|action| action.blocks_update())
        .count()
}

pub fn write_generated_files(root: &Path, files: GeneratedFiles) -> Result<()> {
    for (relative_path, generated_file) in files {
        let path = managed_file_path(root, &relative_path)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        write_generated_file(&path, &generated_file)?;
    }
    Ok(())
}

fn write_text_file_atomically(path: &Path, content: &str) -> Result<()> {
    let temp_path = temp_path_for(path)?;
    let write_result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .with_context(|| format!("failed to create temporary {}", temp_path.display()))?;
        file.write_all(content.as_bytes())
            .with_context(|| format!("failed to write temporary {}", temp_path.display()))?;
        file.sync_all()
            .with_context(|| format!("failed to sync temporary {}", temp_path.display()))?;
        fs::rename(&temp_path, path)
            .with_context(|| format!("failed to replace {}", path.display()))?;
        Ok(())
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }

    write_result.with_context(|| format!("failed to write {}", path.display()))
}

fn temp_path_for(path: &Path) -> Result<PathBuf> {
    let file_name = path
        .file_name()
        .context("managed file path must include a file name")?
        .to_string_lossy();
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    for attempt in 0..100 {
        let temp_name = format!(".{file_name}.forge-tmp-{}-{attempt}", std::process::id());
        let temp_path = parent.join(temp_name);
        if !temp_path.exists() && temp_path.symlink_metadata().is_err() {
            return Ok(temp_path);
        }
    }

    bail!(
        "failed to choose temporary path for managed file {}",
        path.display()
    )
}

pub fn remove_managed_file_if_exists(path: &Path) -> Result<()> {
    let metadata = match path.symlink_metadata() {
        Ok(metadata) => metadata,
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
            ) =>
        {
            return Ok(());
        }
        Err(error) => {
            return Err(error).with_context(|| format!("failed to inspect {}", path.display()));
        }
    };

    let file_type = metadata.file_type();
    if file_type.is_file() || file_type.is_symlink() {
        fs::remove_file(path).with_context(|| format!("failed to remove {}", path.display()))?;
        remove_empty_parent_dir(path)?;
    }

    Ok(())
}

pub fn remove_managed_files_if_exists(root: &Path, relative_paths: Vec<PathBuf>) -> Result<()> {
    for relative_path in relative_paths {
        remove_managed_file_if_exists(&root.join(relative_path))?;
    }

    Ok(())
}

fn remove_empty_parent_dir(path: &Path) -> Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    match fs::remove_dir(parent) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::DirectoryNotEmpty => Ok(()),
        Err(error) => {
            Err(error).with_context(|| format!("failed to remove empty {}", parent.display()))
        }
    }
}

fn replace_symlink(path: &Path, target: &Path) -> Result<()> {
    ensure_safe_managed_path(target)
        .with_context(|| format!("invalid managed symlink target {}", target.display()))?;
    if path.is_dir() && !path.symlink_metadata()?.file_type().is_symlink() {
        bail!(
            "cannot replace directory with managed symlink: {}",
            path.display()
        );
    }

    let temp_path = temp_path_for(path)?;
    let replace_result = (|| -> Result<()> {
        create_managed_link(path, target, &temp_path).with_context(|| {
            format!(
                "failed to create managed link {} -> {}",
                temp_path.display(),
                target.display()
            )
        })?;
        fs::rename(&temp_path, path)
            .with_context(|| format!("failed to replace {}", path.display()))?;
        Ok(())
    })();

    if replace_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }

    replace_result
}

fn create_managed_link(path: &Path, target: &Path, temp_path: &Path) -> std::io::Result<()> {
    match symlink_file(target, temp_path) {
        Ok(()) => Ok(()),
        Err(error) => create_managed_link_fallback(path, target, temp_path, error),
    }
}

#[cfg(unix)]
fn create_managed_link_fallback(
    _path: &Path,
    _target: &Path,
    _temp_path: &Path,
    error: std::io::Error,
) -> std::io::Result<()> {
    Err(error)
}

#[cfg(windows)]
fn create_managed_link_fallback(
    path: &Path,
    target: &Path,
    temp_path: &Path,
    error: std::io::Error,
) -> std::io::Result<()> {
    if !windows_symlink_error_allows_fallback(&error) {
        return Err(error);
    }

    let resolved_target = resolve_managed_link_target(path, target);
    match fs::hard_link(&resolved_target, temp_path) {
        Ok(()) => Ok(()),
        Err(_) => fs::copy(&resolved_target, temp_path).map(|_| ()),
    }
}

#[cfg(windows)]
fn windows_symlink_error_allows_fallback(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::PermissionDenied
        || error.kind() == std::io::ErrorKind::Unsupported
        || error.raw_os_error() == Some(1314)
}

fn managed_link_fallback_matches_target(path: &Path, target: &Path) -> bool {
    #[cfg(windows)]
    {
        let Ok(metadata) = path.symlink_metadata() else {
            return false;
        };
        if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
            return false;
        }

        let resolved_target = resolve_managed_link_target(path, target);
        let Ok(target_metadata) = resolved_target.symlink_metadata() else {
            return false;
        };
        if !target_metadata.file_type().is_file() {
            return false;
        }
        if metadata.len() != target_metadata.len() {
            return false;
        }

        let Ok(existing_contents) = fs::read(path) else {
            return false;
        };
        let Ok(target_contents) = fs::read(resolved_target) else {
            return false;
        };

        existing_contents == target_contents
    }

    #[cfg(not(windows))]
    {
        let _ = (path, target);
        false
    }
}

#[cfg(windows)]
fn resolve_managed_link_target(path: &Path, target: &Path) -> PathBuf {
    let base = path.parent().unwrap_or_else(|| Path::new(""));
    base.join(target)
}

#[cfg(unix)]
fn symlink_file(target: &Path, path: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, path)
}

#[cfg(windows)]
fn symlink_file(target: &Path, path: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(target, path)
}

#[cfg(test)]
mod tests {
    use crate::blueprint::files::{
        GeneratedFile, GeneratedFiles, ManagedFileAction, ManagedFileConflict, plan_generated_files,
    };
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn planner_reports_directory_conflict_for_managed_text_file() {
        let temp = TempDir::new().expect("temp dir should create");
        std::fs::create_dir(temp.path().join("justfile")).expect("directory should create");
        let mut files = GeneratedFiles::new();
        files.insert(PathBuf::from("justfile"), GeneratedFile::text("verify:\n"));

        let actions = plan_generated_files(temp.path(), &files);

        assert_eq!(
            actions,
            vec![ManagedFileAction::Conflict {
                path: PathBuf::from("justfile"),
                reason: ManagedFileConflict::Directory,
            }]
        );
    }

    #[cfg(unix)]
    #[test]
    fn planner_reports_unreadable_conflict_for_managed_text_file() {
        use std::os::unix::fs::PermissionsExt;

        let temp = TempDir::new().expect("temp dir should create");
        let managed_path = temp.path().join("justfile");
        std::fs::write(&managed_path, "existing\n").expect("file should write");
        let original_permissions = std::fs::metadata(&managed_path)
            .expect("file metadata should read")
            .permissions();
        std::fs::set_permissions(&managed_path, std::fs::Permissions::from_mode(0o000))
            .expect("file should become unreadable");
        let mut files = GeneratedFiles::new();
        files.insert(PathBuf::from("justfile"), GeneratedFile::text("expected\n"));

        let actions = plan_generated_files(temp.path(), &files);

        std::fs::set_permissions(&managed_path, original_permissions)
            .expect("file permissions should restore");

        assert_eq!(
            actions,
            vec![ManagedFileAction::Conflict {
                path: PathBuf::from("justfile"),
                reason: ManagedFileConflict::Unreadable,
            }]
        );
    }

    #[cfg(unix)]
    #[test]
    fn planner_reports_broken_symlink_conflict_for_managed_text_file() {
        let temp = TempDir::new().expect("temp dir should create");
        std::os::unix::fs::symlink("missing-justfile", temp.path().join("justfile"))
            .expect("broken symlink should create");
        let mut files = GeneratedFiles::new();
        files.insert(PathBuf::from("justfile"), GeneratedFile::text("expected\n"));

        let actions = plan_generated_files(temp.path(), &files);

        assert_eq!(
            actions,
            vec![ManagedFileAction::Conflict {
                path: PathBuf::from("justfile"),
                reason: ManagedFileConflict::Unreadable,
            }]
        );
    }

    #[test]
    fn planner_reports_directory_conflict_for_managed_symlink() {
        let temp = TempDir::new().expect("temp dir should create");
        std::fs::create_dir(temp.path().join("CLAUDE.md")).expect("directory should create");
        let mut files = GeneratedFiles::new();
        files.insert(
            PathBuf::from("CLAUDE.md"),
            GeneratedFile::symlink("AGENTS.md"),
        );

        let actions = plan_generated_files(temp.path(), &files);

        assert_eq!(
            actions,
            vec![ManagedFileAction::Conflict {
                path: PathBuf::from("CLAUDE.md"),
                reason: ManagedFileConflict::Directory,
            }]
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn planner_relinks_regular_file_for_managed_symlink_on_non_windows() {
        let temp = TempDir::new().expect("temp dir should create");
        std::fs::write(temp.path().join("AGENTS.md"), "shared instructions\n")
            .expect("target should write");
        std::fs::write(temp.path().join("CLAUDE.md"), "shared instructions\n")
            .expect("regular file should write");
        let mut files = GeneratedFiles::new();
        files.insert(
            PathBuf::from("CLAUDE.md"),
            GeneratedFile::symlink("AGENTS.md"),
        );

        let actions = plan_generated_files(temp.path(), &files);

        assert_eq!(
            actions,
            vec![ManagedFileAction::Relink(PathBuf::from("CLAUDE.md"))]
        );
    }

    #[cfg(windows)]
    #[test]
    fn planner_keeps_windows_fallback_copy_for_managed_symlink() {
        let temp = TempDir::new().expect("temp dir should create");
        std::fs::write(temp.path().join("AGENTS.md"), "shared instructions\n")
            .expect("target should write");
        std::fs::write(temp.path().join("CLAUDE.md"), "shared instructions\n")
            .expect("fallback file should write");
        let mut files = GeneratedFiles::new();
        files.insert(
            PathBuf::from("CLAUDE.md"),
            GeneratedFile::symlink("AGENTS.md"),
        );

        let actions = plan_generated_files(temp.path(), &files);

        assert_eq!(
            actions,
            vec![ManagedFileAction::Keep(PathBuf::from("CLAUDE.md"))]
        );
    }

    #[cfg(windows)]
    #[test]
    fn planner_relinks_stale_windows_fallback_copy_for_managed_symlink() {
        let temp = TempDir::new().expect("temp dir should create");
        std::fs::write(temp.path().join("AGENTS.md"), "shared instructions\n")
            .expect("target should write");
        std::fs::write(temp.path().join("CLAUDE.md"), "stale instructions\n")
            .expect("fallback file should write");
        let mut files = GeneratedFiles::new();
        files.insert(
            PathBuf::from("CLAUDE.md"),
            GeneratedFile::symlink("AGENTS.md"),
        );

        let actions = plan_generated_files(temp.path(), &files);

        assert_eq!(
            actions,
            vec![ManagedFileAction::Relink(PathBuf::from("CLAUDE.md"))]
        );
    }

    #[test]
    fn planner_reports_parent_path_conflict_before_create() {
        let temp = TempDir::new().expect("temp dir should create");
        std::fs::write(temp.path().join(".github"), "not a directory\n")
            .expect("parent file should write");
        let mut files = GeneratedFiles::new();
        files.insert(
            PathBuf::from(".github/workflows/ci.yaml"),
            GeneratedFile::text("name: CI\n"),
        );

        let actions = plan_generated_files(temp.path(), &files);

        assert_eq!(
            actions,
            vec![ManagedFileAction::Conflict {
                path: PathBuf::from(".github/workflows/ci.yaml"),
                reason: ManagedFileConflict::ParentNotDirectory,
            }]
        );
    }

    #[test]
    fn planner_reports_unsafe_conflict_for_absolute_managed_path() {
        let temp = TempDir::new().expect("temp dir should create");
        let mut files = GeneratedFiles::new();
        files.insert(
            PathBuf::from("/tmp/forge-escape"),
            GeneratedFile::text("escape\n"),
        );

        let actions = plan_generated_files(temp.path(), &files);

        assert_eq!(
            actions,
            vec![ManagedFileAction::Conflict {
                path: PathBuf::from("/tmp/forge-escape"),
                reason: ManagedFileConflict::UnsafePath,
            }]
        );
    }

    #[test]
    fn planner_reports_unsafe_conflict_for_parent_traversal_managed_path() {
        let temp = TempDir::new().expect("temp dir should create");
        let mut files = GeneratedFiles::new();
        files.insert(
            PathBuf::from("../forge-escape"),
            GeneratedFile::text("escape\n"),
        );

        let actions = plan_generated_files(temp.path(), &files);

        assert_eq!(
            actions,
            vec![ManagedFileAction::Conflict {
                path: PathBuf::from("../forge-escape"),
                reason: ManagedFileConflict::UnsafePath,
            }]
        );
    }

    #[test]
    fn planner_reports_unsafe_conflict_for_absolute_symlink_target() {
        let temp = TempDir::new().expect("temp dir should create");
        let mut files = GeneratedFiles::new();
        files.insert(
            PathBuf::from("CLAUDE.md"),
            GeneratedFile::symlink("/tmp/forge-escape"),
        );

        let actions = plan_generated_files(temp.path(), &files);

        assert_eq!(
            actions,
            vec![ManagedFileAction::Conflict {
                path: PathBuf::from("CLAUDE.md"),
                reason: ManagedFileConflict::UnsafePath,
            }]
        );
    }

    #[test]
    fn planner_reports_unsafe_conflict_for_parent_traversal_symlink_target() {
        let temp = TempDir::new().expect("temp dir should create");
        let mut files = GeneratedFiles::new();
        files.insert(
            PathBuf::from("CLAUDE.md"),
            GeneratedFile::symlink("../AGENTS.md"),
        );

        let actions = plan_generated_files(temp.path(), &files);

        assert_eq!(
            actions,
            vec![ManagedFileAction::Conflict {
                path: PathBuf::from("CLAUDE.md"),
                reason: ManagedFileConflict::UnsafePath,
            }]
        );
    }

    #[test]
    fn remove_managed_file_removes_regular_files_and_empty_parent() {
        let temp = TempDir::new().expect("temp dir should create");
        let file = temp.path().join("docs/index.md");
        std::fs::create_dir(file.parent().expect("file should have parent"))
            .expect("parent should create");
        std::fs::write(&file, "# Docs\n").expect("file should write");

        crate::blueprint::files::remove_managed_file_if_exists(&file)
            .expect("managed file should remove");

        assert!(!file.exists());
        assert!(!temp.path().join("docs").exists());
    }

    #[test]
    fn remove_managed_files_removes_relative_paths_from_root() {
        let temp = TempDir::new().expect("temp dir should create");
        let first = temp.path().join("docs/index.md");
        let second = temp.path().join(".github/workflows/ci.yaml");
        std::fs::create_dir(first.parent().expect("file should have parent"))
            .expect("docs parent should create");
        std::fs::create_dir_all(second.parent().expect("file should have parent"))
            .expect("workflow parent should create");
        std::fs::write(&first, "# Docs\n").expect("first file should write");
        std::fs::write(&second, "name: CI\n").expect("second file should write");

        crate::blueprint::files::remove_managed_files_if_exists(
            temp.path(),
            vec![
                PathBuf::from("docs/index.md"),
                PathBuf::from(".github/workflows/ci.yaml"),
            ],
        )
        .expect("managed files should remove");

        assert!(!first.exists());
        assert!(!second.exists());
        assert!(!temp.path().join("docs").exists());
        assert!(!temp.path().join(".github/workflows").exists());
    }

    #[cfg(unix)]
    #[test]
    fn remove_managed_file_reports_metadata_errors() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let path = PathBuf::from(OsString::from_vec(b"invalid\0path".to_vec()));

        let error = crate::blueprint::files::remove_managed_file_if_exists(&path)
            .expect_err("invalid path should report metadata failure");

        assert!(error.to_string().contains("failed to inspect"));
    }

    #[test]
    fn remove_managed_file_treats_blocked_child_path_as_absent() {
        let temp = TempDir::new().expect("temp dir should create");
        let parent = temp.path().join(".github");
        std::fs::write(&parent, "not a directory\n").expect("parent file should write");

        crate::blueprint::files::remove_managed_file_if_exists(
            &temp.path().join(".github/workflows/ci.yaml"),
        )
        .expect("blocked child path should be absent");

        assert_eq!(
            std::fs::read_to_string(&parent).expect("parent file should remain readable"),
            "not a directory\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn failed_text_write_preserves_existing_file() {
        use std::os::unix::fs::PermissionsExt;

        let temp = TempDir::new().expect("temp dir should create");
        let file = temp.path().join("README.md");
        std::fs::write(&file, "existing\n").expect("file should write");

        let original_permissions = std::fs::metadata(temp.path())
            .expect("temp metadata should read")
            .permissions();
        let mut read_only_permissions = original_permissions.clone();
        read_only_permissions.set_mode(0o500);
        std::fs::set_permissions(temp.path(), read_only_permissions)
            .expect("temp dir should become read-only");

        let result = crate::blueprint::files::write_generated_file(
            &file,
            &GeneratedFile::text("replacement\n"),
        );

        std::fs::set_permissions(temp.path(), original_permissions)
            .expect("temp dir permissions should restore");

        let error = result.expect_err("write should fail without temp-file directory permission");
        assert!(error.to_string().contains("failed to write"));
        assert_eq!(
            std::fs::read_to_string(&file).expect("file should remain readable"),
            "existing\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn failed_symlink_replacement_preserves_existing_link() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let temp = TempDir::new().expect("temp dir should create");
        let link = temp.path().join("CLAUDE.md");
        crate::blueprint::files::symlink_file(PathBuf::from("OLD.md").as_path(), &link)
            .expect("existing symlink should create");
        let invalid_target = PathBuf::from(OsString::from_vec(b"AGENTS\0.md".to_vec()));

        let result = crate::blueprint::files::write_generated_file(
            &link,
            &GeneratedFile::symlink(invalid_target),
        );

        let error = result.expect_err("invalid symlink target should fail");
        assert!(error.to_string().contains("failed to create managed link"));
        assert_eq!(
            std::fs::read_link(&link).expect("existing symlink should remain readable"),
            PathBuf::from("OLD.md")
        );
    }

    #[cfg(windows)]
    #[test]
    fn resolve_managed_link_target_uses_link_parent_directory() {
        let link = PathBuf::from("nested/CLAUDE.md");
        let resolved = crate::blueprint::files::resolve_managed_link_target(
            &link,
            std::path::Path::new("AGENTS.md"),
        );

        assert_eq!(resolved, PathBuf::from("nested/AGENTS.md"));
    }

    #[cfg(windows)]
    #[test]
    fn windows_fallback_link_match_accepts_only_matching_regular_files() {
        let temp = TempDir::new().expect("temp dir should create");
        let target = temp.path().join("AGENTS.md");
        let link = temp.path().join("CLAUDE.md");
        std::fs::write(&target, "shared instructions\n").expect("target should write");
        std::fs::write(&link, "shared instructions\n").expect("fallback file should write");

        assert!(
            crate::blueprint::files::managed_link_fallback_matches_target(
                &link,
                std::path::Path::new("AGENTS.md")
            )
        );

        std::fs::write(&link, "different\n").expect("fallback file should update");
        assert!(
            !crate::blueprint::files::managed_link_fallback_matches_target(
                &link,
                std::path::Path::new("AGENTS.md")
            )
        );
    }

    #[cfg(unix)]
    #[test]
    fn remove_managed_file_removes_broken_symlinks() {
        let temp = TempDir::new().expect("temp dir should create");
        let link = temp.path().join("nested/CLAUDE.md");
        std::fs::create_dir(link.parent().expect("link should have parent"))
            .expect("parent should create");
        crate::blueprint::files::symlink_file(PathBuf::from("AGENTS.md").as_path(), &link)
            .expect("symlink should create");

        crate::blueprint::files::remove_managed_file_if_exists(&link)
            .expect("managed symlink should remove");

        assert!(link.symlink_metadata().is_err());
        assert!(!temp.path().join("nested").exists());
    }
}
