use anyhow::Result;

fn main() -> Result<()> {
    let launch_spec =
        capsa_core::daemon::launch_spec_args::parse_launch_spec_args(std::env::args().skip(1))?;
    capsa_vmm::start_vm(&launch_spec)
}
