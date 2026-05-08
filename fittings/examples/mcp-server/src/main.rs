use std::process;

mod mcpfit_example;

#[tokio::main]
async fn main() {
    let exit_code = match mcpfit_example::build_server().run_entrypoint().await {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("mcp-server serve error: {error}");
            1
        }
    };
    process::exit(exit_code);
}
