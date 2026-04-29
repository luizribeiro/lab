use std::path::Path;
use std::process::Command;

use crate::{ObservationMode, SandboxSpec};

mod paths;
mod policy;
mod seatbelt;

use policy::build_policy;

/// Builds a `Command` that runs `program` under `sandbox-exec` with the
/// seatbelt profile passed inline via `-p`.
pub(crate) fn build_sandbox_command(
    spec: &SandboxSpec,
    private_tmp: &Path,
    program: &Path,
) -> Command {
    let profile = render_profile(program, spec, private_tmp);

    let mut command = Command::new("/usr/bin/sandbox-exec");
    if let Some(run_id) = run_id_param(&spec.observation) {
        command.arg("-D").arg(format!("RUN_ID={run_id}"));
    }
    command
        .arg("-p")
        .arg(&profile)
        .arg(program)
        .env("TMPDIR", private_tmp)
        .env("TMP", private_tmp)
        .env("TEMP", private_tmp);
    command
}

fn render_profile(program: &Path, spec: &SandboxSpec, private_tmp: &Path) -> String {
    if matches!(spec.observation, ObservationMode::AllowAllWithRunId(_)) {
        return concat!(
            "(version 1)\n",
            "(allow default (with report) (with message (param \"RUN_ID\")))\n",
        )
        .to_string();
    }
    build_policy(program, spec, private_tmp).into()
}

fn run_id_param(mode: &ObservationMode) -> Option<&str> {
    match mode {
        ObservationMode::None => None,
        ObservationMode::AllowAllWithRunId(id) | ObservationMode::DenyTraceWithRunId(id) => {
            Some(id.as_str())
        }
    }
}
