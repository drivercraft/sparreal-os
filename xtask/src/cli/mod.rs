use anyhow::Result;
use clap::{Parser, Subcommand};

mod commands;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Test(TestArgs),
}

#[derive(Parser)]
pub struct TestArgs {
    /// 目标架构 (aarch64, aarch64_el2, loongarch64, riscv64, x86_64)
    #[arg(long)]
    pub target: String,
    /// 测试套件 (timer, smp, hello, async, simple_bare_test)
    #[arg(long, default_value = "timer")]
    pub suite: String,
    /// 启用 GDB 调试模式
    #[arg(long, short)]
    pub debug: bool,
}

pub(crate) async fn run_cli() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Test(args) => {
            commands::run_test(args).await?;
        }
    }

    Ok(())
}
