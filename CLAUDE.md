# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

Sparreal OS（雀实操作系统）是一个用 Rust 2024 Edition 编写的轻量级实时操作系统（RTOS），支持 AArch64 和 LoongArch64 架构。

核心特性：

- 纯 Rust 实现，#![no_std] 环境
- 基于平台抽象层（HAL）的分层架构设计
- 内置单核异步执行器
- 完整的驱动框架集成（rdrive）

## 构建和测试

### 核心工具

- **ostool**: 主要构建工具（替代传统 Make）
- **QEMU**: 模拟器测试环境

安装 ostool：

```bash
cargo install ostool
```

### 常用命令

构建内核：

```bash
# 默认构建 AArch64
ostool build

# 构建 LoongArch64
ostool build --config loongarch64.toml
```

运行测试：

```bash
# QEMU 快速测试
ostool run qemu

# QEMU 调试模式（GDB）
ostool run qemu -d

# U-Boot 引导（需要连接串口）
ostool run uboot
```

运行测试套件：

```bash
# AArch64 测试
./scripts/test_aarch64.sh

# LoongArch64 测试
./scripts/test_loongarch64.sh

# 所有测试
./scripts/test_all.sh
```

运行单个测试（示例：timer 测试）：

```bash
ostool run -c ./test-suit/timer/aarch64.toml qemu -q ./test-suit/timer/qemu-aarch64.toml
```

### 代码质量检查

```bash
# 格式检查
cargo fmt --all -- --check

# Clippy 检查
cargo clippy --target aarch64-unknown-none-softfloat -- -D warnings
cargo clippy --target loongarch64-unknown-none-softfloat -- -D warnings

# 运行所有测试
cargo test --workspace
```

### GDB 调试

VSCode 调试配置已包含在 `.vscode/launch.json`：

1. 运行 `ostool run qemu -d` 启动 GDB 服务器
2. 在 VSCode 中选择 "KDebug cppdbg" 配置
3. 按 F5 开始调试

## 代码架构

### 分层设计

```text
应用层（apps/、test-suit/）
    ↓
sparreal-kernel（内核核心）
    ↓
平台抽象层（HAL Trait）
    ↓
somehal + sparreal-rt（实现层）
    ↓
硬件层（QEMU/真实硬件）
```

### Workspace 结构

- **crates/**: 核心组件库
  - `sparreal-kernel/`: 主内核实现
  - `someboot/`: 引导加载程序（EFI Stub 支持）
  - `sparreal-macros/`: 内核宏定义（#[api_impl] 等）
  - `mmio-api/`: 内存映射 I/O 抽象
  - `dma-api/`: DMA 操作抽象
  - `ranges-ext2/`: 范围操作扩展库
  - `page-table-generic/`: 通用页表管理
  - `kernutil/`: 内核工具库
  - `bare-test/`: 裸机测试框架

- **platform/**: 平台实现
  - `somehal/`: 硬件抽象层实现（v0.5.1）
  - `sparreal-rt/`: 运行时实现

- **apps/**: 示例应用程序
- **test-suit/**: 测试套件
- **xtask/**: 构建任务工具
- **build-config/**: 构建配置文件

### 平台抽象层（HAL）接口

内核通过 `sparreal-kernel/src/hal/al.rs` 中定义的 trait 与硬件解耦：

- **Platform**: 平台接口（关机、中断控制等）
- **Memory**: 内存管理（页表、地址转换）
- **Cpu**: CPU 管理（时钟、中断）
- **Console**: 控制台接口

实现位置：`platform/somehal/src/arch/`（按架构分离）

### 架构支持

| 架构        | 状态     | 实现位置                                 |
| :---------- | :------- | :--------------------------------------- |
| AArch64     | 完整支持 | `platform/somehal/src/arch/aarch64/`     |
| LoongArch64 | 开发中   | `platform/somehal/src/arch/loongarch64/` |

### feature 标志

`somehal` 包的特性：

- `efi`: EFI Stub 支持
- `hv`: 虚拟化支持
- `mmu`: 内存管理单元支持
- `uspace`: 用户态支持（依赖 mmu）

## 启动流程

### AArch64

1. UEFI 固件加载内核
2. `kernel_entry` 汇编入口
3. 设置页表和栈指针
4. 跳转到 Rust 主函数

### LoongArch64

1. UEFI 加载 `BOOTLOONGARCH64.EFI`
2. `efi_pe_entry` Rust 入口点
3. 设置直接映射窗口（DMW）
4. 启用分页并跳转到虚拟地址

详细文档：`doc/loongarch/Devlog.md`

## 适配新硬件

1. 在 `platform/somehal/src/arch/` 下创建新架构目录
2. 实现 HAL trait（Platform、Memory、Cpu、Console）
3. 使用 `#[api_impl]` 宏标记实现
4. 在 `build-config/` 下创建构建配置
5. 使用 `sparreal_rt::entry` 宏定义入口点

示例：

```rust
use sparreal_kernel::platform_if::Platform;
use sparreal_macros::api_impl;

#[api_impl]
impl Platform for PlatformImpl {
    unsafe fn wait_for_interrupt() {
        // 平台特定的 WFI 实现
    }
    // ... 其他方法
}
```

## 关键依赖

核心依赖：

- `rdrive` 0.18: 驱动框架
- `page-table-generic` 0.7: 通用页表管理
- `buddy_system_allocator` 0.11: 伙伴分配器
- `spin` 0.10: 自旋锁
- `heapless` 0.9: 无堆数据结构
- `log` 0.4: 日志接口

架构相关依赖（AArch64）：

- `aarch64-cpu` 11: CPU 寄存器访问
- `arm-gic-driver` 0.16: GIC 中断控制器

## 提交规范

使用 Conventional Commits：

- `feat:` 新功能
- `fix:` Bug 修复
- `refactor:` 重构
- `docs:` 文档更新
- `test:` 测试相关
- `chore:` 构建/工具链

## CI/CD

- GitHub Actions 自动测试：`.github/workflows/test.yml`
- 自动发布：`.github/workflows/release-plz.yml`
- 多架构并行测试：AArch64 和 LoongArch64
