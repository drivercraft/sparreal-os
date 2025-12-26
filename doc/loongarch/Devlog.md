# 开发记录

## 启动过程

LoongArch 架构的启动过程与其他主流架构类似，但也有其独特之处。以下是 LoongArch 架构的启动过程的简要描述：


## 虚拟化

### 概述

LoongArch 架构的虚拟化扩展被称为 **LVZ (Loongson Virtualization)**，它是 LoongArch 指令集的五个主要模块之一：

1. **基础指令集** (Loongson Base)
2. **二进制翻译扩展** (LBT)
3. **向量扩展** (LSX - 128位)
4. **高级向量扩展** (LASX - 256位)
5. **虚拟化扩展** (LVZ) ← 当前主题

LVZ 提供了硬件级别的虚拟化支持，使 LoongArch 处理器能够高效运行虚拟机。这一扩展主要在龙芯 3 系列处理器（如 3A6000）中实现。

**发展里程碑**：

- 2023 年 10 月：龙芯宣布为 Linux 内核 6.7 增加 KVM 虚拟化支持
- 2024 年 2 月：Linux 6.7 正式合并 LoongArch KVM 支持
- 2024 年：OpenCloudOS Stream 23 完整支持 LSX、LASX、LVZ 和 LBT 指令集

### CPU 运行模式

实现了 LVZ 虚拟化扩展的处理器支持两个运行模式：

#### Host 模式

- 由 Hypervisor（虚拟机监控器）使用
- 拥有对硬件的完全控制权
- 负责管理和调度虚拟机
- 在非虚拟化场景下，直接运行操作系统（如 Linux 内核在 PLV0，用户态在 PLV3）

#### Guest 模式

- 运行客户机操作系统的模式
- 受 Host 模式下 Hypervisor 的控制
- 通过 **hvcl** (Hypercall) 指令可以主动陷入 Host 模式
- 在诸多方面受限，但仍可通过 GCSR 寄存器组管理自己的特权资源

**特权级说明**：每个模式（Host/Guest）都有四个特权级（PLV0-PLV3），由 `CSR.CRMD` 寄存器的 `PLV` 字段确定。

### 虚拟化专用寄存器

LVZ 扩展引入了一组新的 CSR 寄存器用于控制虚拟化：

| 寄存器编号 | 名称 | 用途 |
| :--- | :--- | :--- |
| 0x15 | `GTLBC` | 客户机 TLB 控制 (Guest TLB Control) |
| 0x16 | `TRGP` | TLBRD 读 Guest 项 |
| 0x50 | `GSTAT` | 客户机状态 (Guest Status) |
| 0x51 | `GCTL` | 客户机控制 (Guest Control) |
| 0x52 | `GINTC` | 客户机中断控制 (Guest Interrupt Control) |
| 0x53 | `GCNTC` | 客户机计数器补偿 (Guest Counter Compensation) |

#### GCSR 寄存器组

在虚拟化 LoongArch 处理器中，还有一套独立的 **GCSR (Guest Control and Status Register)** 寄存器组：

- **目的**：供 Guest 模式下的虚拟机操作系统使用
- **优势**：让虚拟机有自己的特权资源和对应管理，避免与 Hypervisor 的特权资源冲突
- **性能**：减少虚拟机陷入 Hypervisor 的次数
- **控制**：虚拟机对 GCSR 的操作仍可被 Hypervisor 监控和拦截（LVZ 允许 Hypervisor 自由选择拦截策略）

### 虚拟化异常

LVZ 定义了以下虚拟化相关的异常：

| 异常码 | 子码 | 缩写 | 触发原因 |
| :--- | :--- | :--- | :--- |
| 22 | - | **GSPR** | 客户机敏感特权资源异常，由 `cpucfg`、`idle`、`cacop` 指令触发，或访问不存在的 GCSR/IOCSR 时触发 |
| 23 | - | **HVC** | Hypercall 超级调用，由 `hvcl` 指令触发，主动陷入 Hypervisor |
| 24 | 0 | **GCM** | 客户机 GCSR 软件修改异常 |
| 24 | 1 | **GCHC** | 客户机 GCSR 硬件修改异常 |

### 模式切换流程

#### 进入 Guest 模式 (switch_to_guest)

基于 Linux KVM 实现，进入 Guest 模式的步骤如下：

1. **清空异常向量分离**：设置 `CSR.ECFG.VS = 0`（所有异常共用一个入口地址）
2. **加载客户机异常入口**：从 Hypervisor 读取 guest eentry → 写入 `CSR.EENTRY`
3. **加载客户机返回地址**：从 Hypervisor 读取 guest era (GPC) → 写入 `CSR.ERA`
4. **保存 Host 页表**：读取 `CSR.PGDL` 并保存到 Hypervisor
5. **加载 Guest 页表**：从 Hypervisor 加载 guest pgdl → `CSR.PGDL`
6. **设置客户机 ID**：读取 `CSR.GSTAT.GID` 和 `CSR.GTLBC.TGID` → 写入 `CSR.GTLBC`
7. **开启 Host 中断**：设置 `CSR.PRMD.PIE = 1`
8. **设置进入 Guest 模式**：设置 `CSR.GSTAT.PGM = 1`（使 `ertn` 指令进入 guest mode）
9. **恢复客户机寄存器**：将 Hypervisor 中保存的客户机通用寄存器（GPRS）恢复到硬件寄存器
10. **执行 `ertn` 指令**：正式进入 Guest 模式

#### 处理 Guest 异常 (kvm_exc_entry)

当 Guest 模式下发生异常时，处理流程如下：

1. **保存客户机现场**：保存 Guest 的通用寄存器（GPRS）
2. **保存状态寄存器**：
   - `CSR.ESTAT` → host ESTAT
   - `CSR.ERA` → GPC (Guest PC)
   - `CSR.BADV` → host BADV（出错虚地址）
   - `CSR.BADI` → host BADI（出错指令）
3. **恢复 Host 配置**：
   - 写入 Host `ECFG` → `CSR.ECFG`
   - 写入 Host `EENTRY` → `CSR.EENTRY`
   - 写入 Host `PGD` → `CSR.PGDL`
4. **关闭 Guest 模式**：清零 `CSR.GSTAT.PGM`
5. **清空客户机 ID**：清空 `GTLBC.TGID` 域
6. **恢复 KVM per-cpu 寄存器**
7. **跳转到异常处理**：跳转到 `KVM_ARCH_HANDLE_EXIT` 处理具体异常
8. **判断继续运行**：
   - 若返回值 ≤ 0：继续运行 Host
   - 若返回值 > 0：准备再次进入 Guest（保存 percpu 寄存器到 `CSR.KSAVE`）
9. **跳转到 `switch_to_guest`**

### vCPU 上下文切换

根据 LoongArch 函数调用规范，vCPU 上下文切换需要保存的寄存器包括：

**通用寄存器**：

- `$s0` - `$s8`：静态寄存器（被调用者保存）
- `$s9` (`$fp`)：栈帧指针 / 静态寄存器
- `$sp` (`$r3`)：栈指针
- `$ra` (`$r1`)：返回地址

**浮点寄存器**（如果使用）：

- `$fs0` - `$fs7`：静态浮点寄存器（被调用者保存）

### 技术参考

**官方文档**：

- [龙芯架构参考手册 卷三：虚拟化扩展](https://loongson.github.io/LoongArch-Documentation/)

**开源实现**：

- [Linux KVM LoongArch 源码](https://github.com/torvalds/linux/blob/master/arch/loongarch/kvm/)
- [hvisor 虚拟化文档](https://hvisor.syswonder.org/chap04/subchap01/LoongArchVirtualization.html)

**相关资源**：

- [龙芯 KVM 虚拟化官方页面](https://www.loongnix.cn/zh/cloud/kvm/)
- [在 QEMU 上调试 Loongson 内核](https://utopianfuture.github.io/kernel/debug-loongarch-kernel-in-qemu.html)

