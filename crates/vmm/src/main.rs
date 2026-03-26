use anyhow::Result;

fn main() -> Result<()> {
    let launch_spec =
        capsa_core::daemon::vmm::args::parse_launch_spec_args(std::env::args().skip(1))?;
    capsa_core::start_vm(&launch_spec)
}
