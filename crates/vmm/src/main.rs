use anyhow::Result;

fn main() -> Result<()> {
    let launch_spec = capsa_spec::parse_launch_spec_args(std::env::args().skip(1))?;
    capsa_vmm::start_vm(&launch_spec)
}
