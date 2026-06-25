pub fn install_forge_step() -> &'static str {
    include_str!("templates/shared/install-forge-step.yaml.j2")
}

pub fn setup_uv_step() -> &'static str {
    include_str!("templates/shared/setup-uv-step.yaml.j2")
}

pub fn uv_sync_locked_step() -> String {
    crate::blueprint::template_engine::render_template("shared/uv-sync-locked-step.yaml.j2", ())
}

pub fn uv_lock_check_step() -> String {
    crate::blueprint::template_engine::render_template("shared/uv-lock-check-step.yaml.j2", ())
}

pub fn uv_run_locked_step(command: &str) -> String {
    crate::blueprint::template_engine::render_template(
        "shared/uv-run-locked-step.yaml.j2",
        serde_json::json!({ "command": command }),
    )
}

pub fn read_only_checkout_step() -> &'static str {
    include_str!("templates/shared/read-only-checkout-step.yaml.j2")
}

pub fn read_only_permissions() -> String {
    format!(
        "{}\n",
        crate::blueprint::template_engine::render_template(
            "shared/read-only-permissions.yaml.j2",
            ()
        )
    )
}

pub fn cancel_redundant_ci_concurrency() -> String {
    format!(
        "{}\n",
        crate::blueprint::template_engine::render_template(
            "shared/cancel-redundant-ci-concurrency.yaml.j2",
            (),
        )
    )
}

pub fn serialized_sync_concurrency() -> String {
    format!(
        "{}\n",
        crate::blueprint::template_engine::render_template(
            "shared/serialized-sync-concurrency.yaml.j2",
            (),
        )
    )
}

pub fn serialized_ref_concurrency() -> String {
    format!(
        "{}\n",
        crate::blueprint::template_engine::render_template(
            "shared/serialized-ref-concurrency.yaml.j2",
            (),
        )
    )
}

pub fn serialized_release_concurrency() -> String {
    format!(
        "{}\n",
        crate::blueprint::template_engine::render_template(
            "shared/serialized-release-concurrency.yaml.j2",
            (),
        )
    )
}

pub fn job_timeout() -> String {
    crate::blueprint::template_engine::render_template("shared/job-timeout.yaml.j2", ())
}

pub fn render_forge_sync_workflow() -> String {
    crate::blueprint::template_engine::render_template(
        "shared/forge-sync.yaml.j2",
        serde_json::json!({
            "serialized_sync_concurrency": serialized_sync_concurrency(),
            "job_timeout": job_timeout(),
            "read_only_checkout_step": read_only_checkout_step(),
            "setup_uv_step": setup_uv_step(),
            "install_forge_step": install_forge_step(),
        }),
    )
}

#[cfg(test)]
mod tests {
    use crate::blueprint::github_actions::{
        cancel_redundant_ci_concurrency, install_forge_step, job_timeout, read_only_checkout_step,
        read_only_permissions, render_forge_sync_workflow, serialized_ref_concurrency,
        serialized_release_concurrency, serialized_sync_concurrency, setup_uv_step,
        uv_lock_check_step, uv_run_locked_step, uv_sync_locked_step,
    };

    #[test]
    fn forge_ci_steps_install_and_check_managed_infrastructure() {
        assert!(install_forge_step().contains("cargo install --git"));
        assert!(install_forge_step().contains("--locked forge"));
        assert_eq!(
            read_only_permissions(),
            "permissions:\n  contents: read\n\n"
        );
        assert_eq!(
            cancel_redundant_ci_concurrency(),
            "concurrency:\n  group: ${{ github.workflow }}-${{ github.ref }}\n  cancel-in-progress: true\n\n"
        );
        assert_eq!(
            serialized_sync_concurrency(),
            "concurrency:\n  group: ${{ github.workflow }}\n  cancel-in-progress: false\n\n"
        );
        assert_eq!(
            serialized_ref_concurrency(),
            "concurrency:\n  group: ${{ github.workflow }}-${{ github.ref }}\n  cancel-in-progress: false\n\n"
        );
        assert_eq!(
            serialized_release_concurrency(),
            "concurrency:\n  group: ${{ github.workflow }}-${{ github.event.release.id }}\n  cancel-in-progress: false\n\n"
        );
        assert_eq!(job_timeout(), "    timeout-minutes: 20\n");
        assert!(
            setup_uv_step().contains("astral-sh/setup-uv@d0cc045d04ccac9d8b7881df0226f9e82c39688e")
        );
        assert!(setup_uv_step().contains("enable-cache: true"));
        assert!(setup_uv_step().contains("uv.lock"));
        assert_eq!(
            uv_sync_locked_step(),
            "      - run: uv sync --all-groups --locked\n"
        );
        assert_eq!(uv_lock_check_step(), "      - run: uv lock --check\n");
        assert_eq!(
            uv_run_locked_step("prek run --all-files"),
            "      - run: uv run --locked prek run --all-files\n"
        );
        assert!(
            read_only_checkout_step()
                .contains("actions/checkout@de0fac2e4500dabe0009e67214ff5f5447ce83dd")
        );
        assert!(read_only_checkout_step().contains("persist-credentials: false"));
    }

    #[test]
    fn forge_sync_workflow_serializes_runs_without_canceling_in_progress_syncs() {
        let workflow = render_forge_sync_workflow();

        assert!(workflow.contains(&serialized_sync_concurrency()));
        assert!(workflow.contains("  cancel-in-progress: false\n\npermissions:"));
    }

    #[test]
    fn forge_sync_workflow_runs_sync_and_opens_pull_request() {
        let workflow = render_forge_sync_workflow();

        assert!(workflow.contains("name: forge-sync"));
        assert!(workflow.contains(install_forge_step()));
        assert!(workflow.contains("id: forge_sync"));
        assert!(workflow.contains("run: forge sync --path . --yes --github-output"));
        assert!(workflow.contains("- Runs `forge sync --path . --yes`"));
        assert!(!workflow.contains("python3"));
        assert!(!workflow.contains("tomllib"));
        assert!(!workflow.contains("<<'PY'"));
        assert!(!workflow.contains("Detect lockfile-relevant metadata changes"));
        assert!(workflow.contains("if: steps.forge_sync.outputs.lockfile == 'true'"));
        assert!(workflow.contains("run: uv lock"));
        assert!(workflow.contains(
            "- Runs `uv lock` only when Forge reports lockfile-relevant metadata changed"
        ));
        assert!(workflow.contains("actions/checkout@de0fac2e4500dabe0009e67214ff5f5447ce83dd"));
        assert!(workflow.contains("persist-credentials: false"));
        assert!(
            workflow.contains(
                "peter-evans/create-pull-request@5f6978faf089d4d20b00c7766989d076bb2fc7f1"
            )
        );
        assert!(workflow.contains("pull-requests: write"));
        assert!(workflow.contains(&job_timeout()));
        assert!(workflow.contains("Forge-managed infrastructure"));
        assert!(!workflow.contains("template-managed"));
    }
}
