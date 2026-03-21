#![cfg_attr(target_os = "none", no_main)]
#![cfg_attr(target_os = "none", no_std)]

#[cfg(target_os = "none")]
mod lang;

#[cfg(not(target_os = "none"))]
mod cli;

#[cfg(not(target_os = "none"))]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    cli::run_cli().await?;
    Ok(())
}
