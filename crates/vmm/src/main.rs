use anyhow::{Context, Result};

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);

    let flag = args.next();
    let config_json = args.next();

    if flag.as_deref() != Some("--vm-config-json") || config_json.is_none() || args.next().is_some()
    {
        anyhow::bail!("usage: capsa-vmm --vm-config-json <json>");
    }

    let config: capsa_core::VmConfig = serde_json::from_str(
        config_json
            .as_deref()
            .expect("checked above: config json is present"),
    )
    .context("failed to parse VM config JSON")?;

    capsa_core::start_vm(&config)
}
