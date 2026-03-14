use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use dialoguer::{Confirm, Input};

use crate::blueprint::python_library::{ProjectConfig, render_project_files};
use crate::cli::{GithubVisibility, NewArgs};

pub fn run(args: NewArgs) -> Result<()> {
    let config = gather_project_config(&args)?;
    config.validate()?;

    let destination = destination_path(&config, &args.path)?;
    ensure_destination_is_empty(&destination)?;

    write_project_files(&destination, &config)?;
    initialize_git_repository(&destination, args.github)?;

    if args.github {
        create_github_repo(
            &destination,
            &config.project_name,
            args.github_owner,
            args.github_visibility,
            args.yes,
        )?;
    }

    println!("Generated project at {}", destination.display());
    Ok(())
}

fn gather_project_config(args: &NewArgs) -> Result<ProjectConfig> {
    let mut project_name = args.project_name.clone();
    let mut package_name = args.package_name.clone();
    let mut description = args.description.clone();
    let mut author_name = args.author_name.clone();
    let mut author_email = args.author_email.clone();

    if !args.yes {
        if project_name.is_none() {
            project_name = Some(Input::new().with_prompt("Project name").interact_text()?);
        }
        if package_name.is_none() {
            package_name = Some(Input::new().with_prompt("Package name").interact_text()?);
        }
        if description.is_none() {
            description = Some(Input::new().with_prompt("Description").interact_text()?);
        }
        if author_name.is_none() {
            author_name = Some(Input::new().with_prompt("Author name").interact_text()?);
        }
        if author_email.is_none() {
            author_email = Some(Input::new().with_prompt("Author email").interact_text()?);
        }
    }

    Ok(ProjectConfig {
        project_name: require_field("project-name", project_name)?,
        package_name: require_field("package-name", package_name)?,
        description: require_field("description", description)?,
        author_name: require_field("author-name", author_name)?,
        author_email: require_field("author-email", author_email)?,
        license: args.license.clone(),
        python_min: args.python_min.clone(),
        docs: args.docs,
        codecov: args.codecov,
        pypi_publish: args.pypi_publish,
    })
}

fn require_field(name: &str, value: Option<String>) -> Result<String> {
    value.ok_or_else(|| anyhow::anyhow!("--{} is required (or run without --yes)", name))
}

fn destination_path(config: &ProjectConfig, path: &Option<PathBuf>) -> Result<PathBuf> {
    match path {
        Some(path) => Ok(path.canonicalize().unwrap_or_else(|_| path.clone())),
        None => Ok(std::env::current_dir()?.join(&config.project_name)),
    }
}

fn ensure_destination_is_empty(path: &Path) -> Result<()> {
    if path.exists() {
        let mut entries = fs::read_dir(path)?;
        if entries.next().is_some() {
            bail!(
                "destination already exists and is not empty: {}",
                path.display()
            );
        }
    }
    fs::create_dir_all(path)
        .with_context(|| format!("failed to create destination {}", path.display()))?;
    Ok(())
}

fn write_project_files(destination: &Path, config: &ProjectConfig) -> Result<()> {
    let files = render_project_files(config);
    for (relative_path, content) in files {
        let path = destination.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, content).with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
}

fn initialize_git_repository(destination: &Path, github_requested: bool) -> Result<()> {
    run_command(destination, "git", &["init", "-b", "main"])?;
    run_command(destination, "git", &["add", "."])?;

    let commit_result = run_command(
        destination,
        "git",
        &["commit", "-m", "chore: initialize project with forge"],
    );

    if github_requested {
        commit_result?;
    }

    Ok(())
}

fn create_github_repo(
    destination: &Path,
    project_name: &str,
    github_owner: Option<String>,
    visibility: GithubVisibility,
    assume_yes: bool,
) -> Result<()> {
    ensure_gh_ready(assume_yes)?;

    let owner = match github_owner {
        Some(owner) => owner,
        None => detect_github_owner()?,
    };
    let repo = format!("{owner}/{project_name}");
    let visibility_flag = match visibility {
        GithubVisibility::Public => "--public",
        GithubVisibility::Private => "--private",
    };

    run_command(
        destination,
        "gh",
        &[
            "repo",
            "create",
            &repo,
            "--source",
            ".",
            "--remote",
            "origin",
            visibility_flag,
            "--push",
        ],
    )?;

    println!("Created and pushed GitHub repo: {repo}");
    Ok(())
}

fn ensure_gh_ready(assume_yes: bool) -> Result<()> {
    if !command_exists("gh") {
        if assume_yes {
            bail!("GitHub CLI is not installed. Install gh first: https://cli.github.com/");
        }

        let should_install = Confirm::new()
            .with_prompt("GitHub CLI (gh) is missing. Install it now?")
            .default(true)
            .interact()?;

        if !should_install {
            bail!("gh is required for GitHub repo creation");
        }

        try_install_gh()?;
    }

    if !gh_is_authenticated()? {
        if assume_yes {
            bail!("GitHub CLI is not authenticated. Run `gh auth login` and retry.");
        }

        let should_login = Confirm::new()
            .with_prompt("Run `gh auth login` now?")
            .default(true)
            .interact()?;

        if !should_login {
            bail!("gh authentication is required for GitHub repo creation");
        }

        let status = Command::new("gh").args(["auth", "login"]).status()?;
        if !status.success() {
            bail!("`gh auth login` failed");
        }
    }

    Ok(())
}

fn try_install_gh() -> Result<()> {
    if command_exists("brew") {
        run_command(Path::new("."), "brew", &["install", "gh"])?;
        return Ok(());
    }
    if command_exists("apt-get") {
        run_command(Path::new("."), "sudo", &["apt-get", "update"])?;
        run_command(Path::new("."), "sudo", &["apt-get", "install", "-y", "gh"])?;
        return Ok(());
    }
    if command_exists("dnf") {
        run_command(Path::new("."), "sudo", &["dnf", "install", "-y", "gh"])?;
        return Ok(());
    }

    bail!(
        "automatic gh installation is unsupported on this OS. Install manually: https://cli.github.com/"
    )
}

fn detect_github_owner() -> Result<String> {
    let output = Command::new("gh")
        .args(["api", "user", "-q", ".login"])
        .output()?;

    if !output.status.success() {
        bail!("failed to detect GitHub owner using `gh api user`");
    }

    let login = String::from_utf8(output.stdout)?.trim().to_string();
    if login.is_empty() {
        bail!("detected empty GitHub owner");
    }
    Ok(login)
}

fn command_exists(command: &str) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}

fn gh_is_authenticated() -> Result<bool> {
    let output = Command::new("gh").args(["auth", "status"]).output()?;
    Ok(output.status.success())
}

fn run_command(cwd: &Path, program: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(program)
        .current_dir(cwd)
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| format!("failed to execute {program}"))?;

    if !status.success() {
        bail!("command failed: {} {}", program, args.join(" "));
    }

    io::stdout().flush().ok();
    Ok(())
}
