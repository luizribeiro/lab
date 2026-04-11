use std::path::Path;
use std::process::Command;

use crate::SandboxSpec;

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
    let policy = build_policy(program, spec, private_tmp);
    let profile: String = policy.into();

    let mut command = Command::new("/usr/bin/sandbox-exec");
    command
        .arg("-p")
        .arg(&profile)
        .arg(program)
        .env("TMPDIR", private_tmp)
        .env("TMP", private_tmp)
        .env("TEMP", private_tmp);
    command
}
