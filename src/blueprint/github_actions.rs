pub fn install_forge_step() -> &'static str {
    include_str!("templates/shared/install-forge-step.yaml.j2")
}

pub fn setup_uv_step() -> &'static str {
    include_str!("templates/shared/setup-uv-step.yaml.j2")
}

pub fn uv_sync_locked_step() -> &'static str {
    "      - run: uv sync --all-groups --locked\n"
}

pub fn uv_lock_check_step() -> &'static str {
    "      - run: uv lock --check\n"
}

pub fn uv_run_locked_step(command: &str) -> String {
    format!("      - run: uv run --locked {command}\n")
}

pub fn read_only_checkout_step() -> &'static str {
    include_str!("templates/shared/read-only-checkout-step.yaml.j2")
}

pub fn forge_update_check_step() -> &'static str {
    "      - run: forge sync --path . --check\n"
}

pub fn read_only_permissions() -> &'static str {
    "permissions:\n  contents: read\n\n"
}

pub fn cancel_redundant_ci_concurrency() -> &'static str {
    "concurrency:\n  group: ${{ github.workflow }}-${{ github.ref }}\n  cancel-in-progress: true\n\n"
}

pub fn serialized_update_concurrency() -> &'static str {
    "concurrency:\n  group: ${{ github.workflow }}\n  cancel-in-progress: false\n\n"
}

pub fn serialized_ref_concurrency() -> &'static str {
    "concurrency:\n  group: ${{ github.workflow }}-${{ github.ref }}\n  cancel-in-progress: false\n\n"
}

pub fn serialized_release_concurrency() -> &'static str {
    "concurrency:\n  group: ${{ github.workflow }}-${{ github.event.release.id }}\n  cancel-in-progress: false\n\n"
}

pub fn job_timeout() -> &'static str {
    "    timeout-minutes: 20\n"
}

pub fn render_forge_update_workflow() -> String {
    crate::blueprint::template_engine::render_template(
        "shared/forge-sync.yaml.j2",
        serde_json::json!({
            "serialized_update_concurrency": serialized_update_concurrency(),
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
        cancel_redundant_ci_concurrency, forge_update_check_step, install_forge_step, job_timeout,
        read_only_checkout_step, read_only_permissions, render_forge_update_workflow,
        serialized_ref_concurrency, serialized_release_concurrency, serialized_update_concurrency,
        setup_uv_step, uv_lock_check_step, uv_run_locked_step, uv_sync_locked_step,
    };

    #[test]
    fn forge_ci_steps_install_and_check_managed_infrastructure() {
        assert!(install_forge_step().contains("cargo install --git"));
        assert!(install_forge_step().contains("--locked forge"));
        assert_eq!(
            forge_update_check_step(),
            "      - run: forge sync --path . --check\n"
        );
        assert_eq!(
            read_only_permissions(),
            "permissions:\n  contents: read\n\n"
        );
        assert_eq!(
            cancel_redundant_ci_concurrency(),
            "concurrency:\n  group: ${{ github.workflow }}-${{ github.ref }}\n  cancel-in-progress: true\n\n"
        );
        assert_eq!(
            serialized_update_concurrency(),
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
    fn forge_update_workflow_serializes_runs_without_canceling_in_progress_updates() {
        let workflow = render_forge_update_workflow();

        assert!(workflow.contains(serialized_update_concurrency()));
        assert!(workflow.contains("  cancel-in-progress: false\n\npermissions:"));
    }

    #[test]
    fn forge_update_workflow_runs_update_and_opens_pull_request() {
        let workflow = render_forge_update_workflow();

        assert!(workflow.contains("name: forge-sync"));
        assert!(workflow.contains(install_forge_step()));
        assert!(workflow.contains("run: forge sync --path ."));
        assert!(workflow.contains("run: uv lock"));
        assert!(workflow.contains("actions/checkout@de0fac2e4500dabe0009e67214ff5f5447ce83dd"));
        assert!(workflow.contains("persist-credentials: false"));
        assert!(
            workflow.contains(
                "peter-evans/create-pull-request@5f6978faf089d4d20b00c7766989d076bb2fc7f1"
            )
        );
        assert!(workflow.contains("pull-requests: write"));
        assert!(workflow.contains(job_timeout()));
        assert!(workflow.contains("Forge-managed infrastructure"));
        assert!(!workflow.contains("template-managed"));
    }
}
