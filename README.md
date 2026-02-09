# Sparreal OS

<div align="center">

**雀实操作系统** - "麻雀虽小，五脏俱全"

[![Rust](https://img.shields.io/badge/Rust-2024%20Edition-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
[![CI](https://github.com/drivercraft/sparreal-os/actions/workflows/test.yml/badge.svg)](https://github.com/drivercraft/sparreal-os/actions/workflows/test.yml)
[![crates.io](https://img.shields.io/crates/v/sparreal-kernel.svg)](https://crates.io/crates/sparreal-kernel)
[![GitHub](https://img.shields.io/badge/GitHub-drivercraft%2Fsparreal--os-brightgreen.svg)](https://github.com/drivercraft/sparreal-os)

一个用 Rust 编写的轻量级实时操作系统（RTOS）

[快速开始](#快速开始) • [架构](#架构设计) • [平台适配](#平台适配) • [文档](#文档)

</div>

---

## 项目简介

Sparreal OS（雀实操作系统）是一个现代化的、用 Rust 编写的轻量级实时操作系统（RTOS），专为嵌入式系统设计。项目名称源于中文成语"麻雀虽小，五脏俱全"，寓意虽小但功能完备。

### 核心特性

- **纯 Rust 实现** - 采用 Rust 2024 Edition，利用零成本抽象和内存安全特性
- **多架构支持** - 支持 AArch64（ARM64）和 LoongArch64（龙芯）架构
- **无标准库设计** - `#![no_std]`，完全自主控制底层实现
- **平台抽象层** - 通过 trait 系统实现硬件无关的内核设计
- **现代化工具链** - 基于 `ostool` 构建系统，支持 QEMU 快速测试和调试
- **异步支持** - 内置单核异步执行器，支持 Rust 异步生态
- **实时性优化** - 硬件中断处理、WFI 指令等低功耗优化

### 技术亮点

<details>
<summary><b>深度技术细节</b></summary>

- **EFI Stub 支持** - 可作为 EFI 应用程序直接由 UEFI 固件加载
- **GIC v2/v3** - 完整支持 ARM Generic Interrupt Controller
- **LVZ 虚拟化** - LoongArch64 架构的硬件虚拟化扩展支持
- **页表管理** - 4KB 页表，支持内核态和用户态地址空间隔离
- **驱动框架** - 集成 `rdrive` 驱动框架，支持 FDT 和 PCIe 设备枚举

</details>

---

## 快速开始

### 前置要求

- Rust 工具链（nightly channel）
- QEMU 模拟器
- 构建工具 `ostool`

### 环境搭建

```bash
# 克隆仓库（包含子模块）
git clone --recurse-submodules https://github.com/drivercraft/sparreal-os.git
cd sparreal-os

# 安装 ostool 构建工具
cargo install ostool
```

<details>
<summary><b>QEMU 安装</b></summary>

**Windows**: 从 [QEMU 官网](https://www.qemu.org/download/#windows) 下载并添加到 PATH

**Linux**:
```bash
# Ubuntu/Debian
sudo apt-get install qemu-system-arm qemu-system-loongarch64

# Arch Linux
sudo pacman -S qemu-emulators
```

**macOS**:
```bash
brew install qemu
```
</details>

### 构建

```bash
# 默认构建（AArch64）
ostool build

# 构建 LoongArch64 版本
ostool build --config loongarch64.toml
```

### 运行测试

```bash
# QEMU 快速测试
ostool run qemu

# QEMU + GDB 调试模式
ostool run qemu -d
```

<details>
<summary><b>VSCode 调试配置</b></summary>

项目已包含 `.vscode/launch.json` 调试配置：

1. 运行 `ostool run qemu -d` 启动 GDB 服务器
2. 在 VSCode 中选择 "KDebug cppdbg" 配置
3. 按 F5 开始调试

**Windows 用户**需安装 `gdb-multiarch`：
```bash
pacman -S mingw-w64-ucrt-x86_64-toolchain
```
</details>

### 开发板运行

```bash
# U-Boot 引导（需要连接串口）
ostool run uboot
```

---

## 架构设计

### 项目结构

```
sparreal-os/
├── crates/              # 核心组件
│   ├── sparreal-kernel/ # 主内核 (v0.13.1)
│   ├── someboot/        # 引导加载程序（支持 EFI Stub）
│   ├── somehal/         # 硬件抽象层实现
│   ├── sparreal-rt/     # 运行时环境
│   ├── sparreal-macros/ # 内核宏（#[api_impl] 等）
│   ├── mmio-api/        # 内存映射 I/O 抽象
│   ├── dma-api/         # DMA 操作抽象
│   ├── kasm-aarch64/    # AArch64 汇编封装
│   ├── page-table-generic/ # 通用页表管理
│   └── kernutil/        # 内核工具库
├── platform/            # 平台相关代码
│   ├── somehal/         # HAL 实现（v0.5）
│   └── sparreal-rt/     # 运行时实现
├── apps/                # 示例应用程序
├── test-suit/           # 测试套件
│   ├── hello/           # 基础测试
│   ├── timer/           # 定时器测试
│   └── async/           # 异步功能测试
└── xtask/               # 构建任务
```

### 核心架构

Sparreal OS 采用分层架构设计，实现硬件无关的内核：

```
┌─────────────────────────────────────────────────┐
│              应用层（Apps/Test-suit）            │
├─────────────────────────────────────────────────┤
│           sparreal-kernel（内核核心）            │
│  • 内存管理  • 中断处理  • 异步执行器 • 驱动框架 │
├─────────────────────────────────────────────────┤
│              平台抽象层（HAL Trait）             │
│  • Platform  • Memory  • Cpu  • Console         │
├─────────────────────────────────────────────────┤
│        somehal（HAL 实现）+ sparreal-rt         │
│  AArch64 实现  │  LoongArch64 实现              │
├─────────────────────────────────────────────────┤
│              硬件层（QEMU/真实硬件）             │
└─────────────────────────────────────────────────┘
```

### 支持的架构

| 架构 | 状态 | 特性 | QEMU 命令 |
|:-----|:-----|:-----|:----------|
| **AArch64** | 完整支持 | GIC v2/v3, EFI Stub, MMU | `qemu-system-aarch64` |
| **LoongArch64** | 开发中 | LVZ 虚拟化, DMW, EFI Stub | `qemu-system-loongarch64` |

### 关键特性实现

#### 平台抽象层

Sparreal OS 通过 trait 系统实现硬件无关的内核设计：

```rust
// 平台接口（由硬件层实现）
pub trait Platform {
    fn post_allocator();
    fn irq_is_enabled(irq: IrqId) -> bool;
    fn irq_set_enabled(irq: IrqId, enabled: bool);
    fn shutdown() -> !;
    fn fdt_addr() -> Option<NonNull<u8>>;
    fn post_paging();
}

// 使用 #[api_impl] 宏实现接口
#[api_impl]
impl Platform for PlatformImpl {
    unsafe fn wait_for_interrupt() {
        aarch64_cpu::asm::wfi();
    }
    // ... 其他方法
}
```

#### 启动流程

**AArch64**:
1. UEFI 固件加载内核
2. `kernel_entry` 汇编入口
3. 设置页表和栈指针
4. 跳转到 Rust 主函数

**LoongArch64**:
1. UEFI 加载 `BOOTLOONGARCH64.EFI`
2. `efi_pe_entry` Rust 入口点
3. 设置直接映射窗口（DMW）
4. 启用分页并跳转到虚拟地址

详见 [LoongArch 开发日志](doc/loongarch/Devlog.md)

---

## 平台适配

### 适配新硬件

Sparreal OS 设计为易于移植到新平台。适配步骤如下：

#### 1. 实现平台接口

```rust
use sparreal_kernel::platform_if::Platform;
use sparreal_macros::api_impl;

pub struct PlatformImpl;

#[api_impl]
impl Platform for PlatformImpl {
    unsafe fn wait_for_interrupt() {
        // 平台特定的 WFI 实现
    }

    fn fdt_addr() -> Option<NonNull<u8>> {
        // 返回设备树地址（如果使用 FDT）
        Some(NonNull::new(0x4000_0000 as *mut u8)?)
    }

    // ... 实现其他必需方法
}
```

#### 2. 实现内存接口

```rust
use sparreal_kernel::platform_if::Memory;

#[api_impl]
impl Memory for MemoryImpl {
    fn _va(paddr: PhysAddr) -> VirtAddr {
        // 物理地址到内核虚拟地址的转换
    }

    fn page_size() -> usize {
        4096 // 或其他页大小
    }

    // ... 其他方法
}
```

#### 3. 配置构建文件

在 `build-config/` 下创建新配置文件：

```toml
[system.Cargo]
args = ["-Z", "build-std=core,alloc"]
target = "your-architecture-unknown-none"
package = "helloworld"
to_bin = true
```

#### 4. 启动内核

```rust
use sparreal_rt::entry;

#[entry]
fn main() -> ! {
    // 初始化平台
    PlatformImpl::init();

    // 启动内核
    sparreal_kernel::boot::kernel_boot();
}
```

### 现有平台实现

- **AArch64**: `platform/somehal/src/arch/aarch64/`
- **LoongArch64**: `platform/somehal/src/arch/loongarch64/`

---

## 测试与调试

### 测试套件

| 测试 | 描述 | 位置 |
|:-----|:-----|:-----|
| **hello** | 基础启动测试 | `test-suit/hello/` |
| **timer** | 定时器和中断测试 | `test-suit/timer/` |
| **async** | 异步执行器测试 | `test-suit/async/` |
| **simple_bare_test** | 裸机单元测试 | `test-suit/simple_bare_test/` |

### 运行测试

```bash
# 运行所有测试
./scripts/test_all.sh

# 运行 AArch64 测试
./scripts/test_aarch64.sh

# 运行 LoongArch64 测试
./scripts/test_loongarch64.sh
```

### CI/CD

项目使用 GitHub Actions 进行持续集成：

- **自动测试**: 每次 PR 推送到 `main` 分支触发
- **多架构**: 并行测试 AArch64 和 LoongArch64
- **自动发布**: 使用 release-plz 自动发布到 crates.io 和 GitHub Releases

详见 [`.github/workflows/test.yml`](.github/workflows/test.yml)

### 调试技巧

<details>
<summary><b>常见调试问题</b></summary>

**问题**: 断点无法命中
- **解决**: 链接脚本自定义 section 后需要手动调整断点地址
- **参见**: `.vscode/launch.json` 中的 `preRunCommands`

**问题**: LoongArch64 重定位失败
- **解决**: 确保执行两次重定位（物理地址和虚拟地址）
- **参考**: `doc/loongarch/Devlog.md` 的重定位章节

**问题**: GIC 中断不触发
- **解决**: 检查 `irq_set_enabled` 是否正确调用
- **工具**: 使用 `rdif-intc` 驱动框架的调试功能
</details>

---

## 依赖项

### 核心依赖

| 依赖 | 版本 | 用途 |
|:-----|:-----|:-----|
| `log` | 0.4 | 日志接口 |
| `spin` | 0.10 | 自旋锁 |
| `heapless` | 0.9 | 无堆数据结构 |
| `buddy_system_allocator` | 0.11 | 伙伴分配器 |
| `page-table-generic` | 0.7 | 通用页表管理 |
| `rdrive` | 0.18 | 驱动框架 |
| `trait-ffi` | 0.2 | FFI trait 支持 |

### 架构相关依赖

**AArch64**:
- `aarch64-cpu` v11 - CPU 寄存器访问
- `arm-gic-driver` v0.16 - GIC 中断控制器

**LoongArch64**:
- 无额外依赖（使用内联汇编）

---

## 文档

- [LoongArch 开发日志](doc/loongarch/Devlog.md) - 详细的启动流程和虚拟化扩展分析
- [各模块 CHANGELOG](crates/) - 版本更新记录
- [API 文档](https://docs.rs/sparreal-kernel/) - crates.io API 文档

---

## 路线图

- [ ] 完整的 LoongArch64 LVZ 虚拟化支持
- [ ] 多核（SMP）支持
- [ ] 更多驱动程序（网络、存储）
- [ ] 用户态进程支持
- [ ] RISC-V 架构支持

---

## 贡献指南

我们欢迎各种形式的贡献！

### 开发环境

```bash
# 安装开发依赖
cargo install cargo-hack

# 运行 clippy
cargo clippy --all-targets

# 运行格式检查
cargo fmt --all -- --check

# 运行测试
cargo test --workspace
```

### 提交规范

使用语义化提交信息（Conventional Commits）：
- `feat:` 新功能
- `fix:` Bug 修复
- `refactor:` 重构
- `docs:` 文档更新
- `test:` 测试相关
- `chore:` 构建/工具链

### Pull Request 流程

1. Fork 项目
2. 创建功能分支 (`git checkout -b feature/amazing-feature`)
3. 提交更改 (`git commit -m 'feat: add amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 开启 Pull Request

所有 PR 需要通过 CI 检查。

---

<div align="center">

**⭐ 如果这个项目对您有帮助，请给我们一个 Star！**

</div>
