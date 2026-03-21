use anyhow::{Context, Result};
use ostool::{
    build::{
        CargoRunnerKind,
        config::{BuildConfig, BuildSystem, Cargo},
    },
    ctx::{AppContext, PathConfig},
    run::qemu::{RunQemuArgs, run_qemu},
};
use std::path::PathBuf;

use crate::cli::TestArgs;

/// 运行测试
///
/// # 参数
/// * `target` - 目标架构 (aarch64, aarch64_el2, loongarch64, riscv64, x86_64)
/// * `suite` - 测试套件名称 (timer, smp, hello, async, simple_bare_test)
/// * `debug` - 是否启用 GDB 调试模式
pub async fn run_test(args: TestArgs) -> Result<()> {
    // 获取当前工作目录作为 workspace 路径
    let workspace = std::env::current_dir().context("获取当前工作目录失败")?;
    let manifest = workspace.clone();

    let paths = PathConfig {
        workspace: workspace.clone(),
        manifest,
        config: Default::default(),
        artifacts: Default::default(),
    };

    let ctx = AppContext {
        paths,
        debug: args.debug,
        arch: None,
        ..Default::default()
    };

    let args = RunQemuArgs {
        qemu_config: None,
        dtb_dump: false,
        show_output: true,
    };

    run_qemu(ctx, args).await?;

    Ok(())
}
