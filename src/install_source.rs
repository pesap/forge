use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

/// Represents how Forge was installed on the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InstallSource {
    Homebrew,
    Cargo,
    #[cfg(feature = "self-update")]
    StandaloneInstaller,
}

impl InstallSource {
    fn from_path(path: &Path) -> Option<Self> {
        let canonical = path.canonicalize().unwrap_or_else(|_| PathBuf::from(path));
        let components: Vec<_> = canonical.components().map(Component::as_os_str).collect();

        fn contains_sequence(components: &[&OsStr], pattern: &[&OsStr]) -> bool {
            components
                .windows(pattern.len())
                .any(|window| window == pattern)
        }

        let forge = OsStr::new("forge");

        if contains_sequence(&components, &[OsStr::new("Cellar"), forge]) {
            return Some(Self::Homebrew);
        }

        if components
            .windows(2)
            .any(|window| window == [OsStr::new(".cargo"), OsStr::new("bin")])
        {
            return Some(Self::Cargo);
        }

        None
    }

    #[cfg(feature = "self-update")]
    fn is_standalone_installer() -> anyhow::Result<bool> {
        use axoupdater::AxoUpdater;

        let mut updater = AxoUpdater::new_for("forge");
        let updater = updater.load_receipt()?;
        Ok(updater.check_receipt_is_for_this_executable()?)
    }

    pub(crate) fn detect() -> Option<Self> {
        #[cfg(feature = "self-update")]
        if let Ok(true) = Self::is_standalone_installer() {
            return Some(Self::StandaloneInstaller);
        }

        Self::from_path(&std::env::current_exe().ok()?)
    }

    pub(crate) const fn description(self) -> &'static str {
        match self {
            Self::Homebrew => "Homebrew",
            Self::Cargo => "cargo",
            #[cfg(feature = "self-update")]
            Self::StandaloneInstaller => "the standalone installer",
        }
    }

    pub(crate) const fn update_instructions(self) -> &'static str {
        match self {
            Self::Homebrew => "brew update && brew upgrade forge",
            Self::Cargo => "cargo install --locked forge",
            #[cfg(feature = "self-update")]
            Self::StandaloneInstaller => "forge self update",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_homebrew_cellar() {
        assert_eq!(
            InstallSource::from_path(Path::new("/opt/homebrew/Cellar/forge/0.1.0/bin/forge")),
            Some(InstallSource::Homebrew)
        );
    }

    #[test]
    fn detects_cargo_bin() {
        assert_eq!(
            InstallSource::from_path(Path::new("/home/me/.cargo/bin/forge")),
            Some(InstallSource::Cargo)
        );
    }

    #[test]
    fn returns_none_for_unknown_path() {
        assert_eq!(
            InstallSource::from_path(Path::new("/usr/local/bin/forge")),
            None
        );
    }
}
