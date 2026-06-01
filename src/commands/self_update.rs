use anyhow::Result;

use crate::cli::{SelfArgs, SelfCommand};
use crate::ui;

pub fn run(args: SelfArgs) -> Result<()> {
    match args.command {
        SelfCommand::Update => {
            let executable = std::env::current_exe()?;
            let location = executable.display().to_string();

            ui::section("Self update");
            ui::info("install path", &location);

            if location.contains("homebrew") || location.contains("linuxbrew") {
                ui::success("forge appears to be installed with Homebrew");
                ui::next_step("brew upgrade forge");
                return Ok(());
            }

            ui::info("status", "manual update required");
            ui::next_step(&manual_update_command(&location));
        }
    }

    Ok(())
}

fn manual_update_command(location: &str) -> String {
    format!(
        "install the latest release, then replace {}",
        ui::shell_arg(location)
    )
}

#[cfg(test)]
mod tests {
    use crate::commands::self_update::manual_update_command;

    #[test]
    fn manual_update_command_quotes_install_paths_with_spaces() {
        assert_eq!(
            manual_update_command("/tmp/forge bin/forge"),
            "install the latest release, then replace '/tmp/forge bin/forge'"
        );
    }

    #[test]
    fn manual_update_command_leaves_simple_install_paths_unquoted() {
        assert_eq!(
            manual_update_command("/usr/local/bin/forge"),
            "install the latest release, then replace /usr/local/bin/forge"
        );
    }
}
