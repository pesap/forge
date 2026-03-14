use std::process::Command;

use anyhow::Result;

pub fn run() -> Result<()> {
    println!("forge doctor");
    println!("- rust binary: ok");

    let gh_ok = Command::new("gh")
        .arg("--version")
        .status()
        .map(|status| status.success())
        .unwrap_or(false);

    if gh_ok {
        println!("- gh cli: installed");
    } else {
        println!("- gh cli: missing (optional unless using GitHub automation)");
    }

    Ok(())
}
