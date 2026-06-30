use anyhow::Result;

use crate::cli::{SelfArgs, SelfCommand, SelfUpdateArgs};
use crate::install_source::InstallSource;
#[cfg(feature = "self-update")]
use crate::ui;

pub fn run(args: SelfArgs) -> Result<()> {
    match args.command {
        SelfCommand::Update(SelfUpdateArgs {
            target_version,
            token,
            dry_run,
        }) => handle_self_update(target_version, token, dry_run),
    }
}

#[cfg(feature = "self-update")]
fn handle_self_update(version: Option<String>, token: Option<String>, dry_run: bool) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let result = runtime.block_on(self_update(version, token, dry_run));
    runtime.shutdown_background();
    result
}

#[cfg(not(feature = "self-update"))]
fn handle_self_update(
    _version: Option<String>,
    _token: Option<String>,
    _dry_run: bool,
) -> Result<()> {
    anyhow::bail!(cannot_self_update_message());
}

fn cannot_self_update_message() -> String {
    InstallSource::detect()
        .map(|source| {
            format!(
                "forge was installed via {} and cannot self-update. To update, run `{}`",
                source.description(),
                source.update_instructions()
            )
        })
        .unwrap_or_else(|| {
            "forge was installed outside the standalone installer and cannot self-update. \
             Please use your package manager or reinstall forge from the latest release."
                .to_string()
        })
}

#[cfg(feature = "self-update")]
async fn self_update(version: Option<String>, token: Option<String>, dry_run: bool) -> Result<()> {
    use anyhow::Context as _;
    use axoupdater::{AxoUpdater, AxoupdateError, UpdateRequest};

    let mut updater = AxoUpdater::new_for("forge");
    updater.disable_installer_output();

    if let Some(ref token) = token {
        updater.set_github_token(token);
    }

    let Ok(updater) = updater.load_receipt() else {
        anyhow::bail!(cannot_self_update_message());
    };

    let current_version = env!("CARGO_PKG_VERSION")
        .parse()
        .context("failed to parse current forge version")?;
    updater.set_current_version(current_version)?;

    if !updater.check_receipt_is_for_this_executable()? {
        anyhow::bail!(cannot_self_update_message());
    }

    ui::section("Self update");
    ui::info("status", "checking for updates");

    let update_request = version.map_or(UpdateRequest::Latest, UpdateRequest::SpecificTag);
    updater.configure_version_specifier(update_request.clone());

    if dry_run {
        if updater.is_update_needed().await? {
            ui::info(
                "update",
                format!(
                    "would update to {}",
                    requested_version_label(&update_request)
                ),
            );
        } else {
            ui::success(format!(
                "forge is already up to date ({})",
                current_version_label()
            ));
        }
        return Ok(());
    }

    match updater.run().await {
        Ok(Some(result)) => {
            let direction = if result
                .old_version
                .as_ref()
                .is_some_and(|old_version| *old_version > result.new_version)
            {
                "downgraded"
            } else {
                "upgraded"
            };

            let version_information = result.old_version.map_or_else(
                || format!("to v{}", result.new_version),
                |old_version| format!("from v{old_version} to v{}", result.new_version),
            );

            ui::success(format!("{direction} forge {version_information}"));
            ui::info(
                "release",
                format!(
                    "https://github.com/pesap/forge/releases/tag/{}",
                    result.new_version_tag
                ),
            );
        }
        Ok(None) => {
            ui::success(format!(
                "forge is already up to date ({})",
                current_version_label()
            ));
        }
        Err(AxoupdateError::Reqwest(err))
            if err.status() == Some(http::StatusCode::FORBIDDEN) && token.is_none() =>
        {
            anyhow::bail!(
                "GitHub API rate limit exceeded. Provide a token with `forge self update --token <token>` or GITHUB_TOKEN."
            );
        }
        Err(err) => return Err(err.into()),
    }

    Ok(())
}

#[cfg(feature = "self-update")]
fn current_version_label() -> String {
    format!("v{}", env!("CARGO_PKG_VERSION"))
}

#[cfg(feature = "self-update")]
fn requested_version_label(update_request: &axoupdater::UpdateRequest) -> String {
    match update_request {
        axoupdater::UpdateRequest::Latest | axoupdater::UpdateRequest::LatestMaybePrerelease => {
            "the latest version".to_string()
        }
        axoupdater::UpdateRequest::SpecificTag(version)
        | axoupdater::UpdateRequest::SpecificVersion(version) => format!("v{version}"),
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::self_update::cannot_self_update_message;

    #[test]
    fn unavailable_self_update_message_mentions_package_manager_or_release() {
        let message = cannot_self_update_message();
        assert!(message.contains("cannot self-update"));
        assert!(message.contains("update") || message.contains("reinstall"));
    }
}
