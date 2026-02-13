# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.13.2](https://github.com/drivercraft/sparreal-os/compare/sparreal-rt-v0.13.1...sparreal-rt-v0.13.2) - 2026-02-13

### Other

- ✨ feat: 添加 PerCpuData 内存类型，优化内存映射和分配逻辑 ([#19](https://github.com/drivercraft/sparreal-os/pull/19))

## [0.13.1](https://github.com/drivercraft/sparreal-os/compare/sparreal-rt-v0.13.0...sparreal-rt-v0.13.1) - 2026-02-09

### Other

- release ([#12](https://github.com/drivercraft/sparreal-os/pull/12))

## [0.13.0](https://github.com/drivercraft/sparreal-os/compare/sparreal-rt-v0.12.2...sparreal-rt-v0.13.0) - 2026-02-09

### Added

- 重构内存管理模块，添加页表项结构及相关功能，优化页表操作逻辑
- 添加用户页表管理功能，更新相关接口以支持用户态页表操作
- 更新内核启动逻辑，添加启动时的 logo 输出
- 更新 heapless 依赖版本，重命名 iomap 为 ioremap，添加 iounmap 方法以支持 I/O 内存映射
- 重构内存管理相关代码，添加 PageTable 操作接口，优化映射逻辑并移除冗余模块
- integrate ranges-ext and num-align for improved memory management
- 更新主函数以显示 SparrealOS 标志
- 重构内存分配器，更新全局分配器名称并优化相关实现
- 添加 Sparreal OS 各模块的架构文档，包括内核、硬件抽象层、平台运行时、异步和定时器测试套件
- 重命名 MMU 设置函数为 enable_paging，更新相关调用以反映新名称
- 添加 MMU 设置功能，更新相关接口以支持内存管理
- 重构系统定时器接口，添加 IRQ 启用、禁用及状态检查功能
- 重构内存管理，添加页表信息结构，更新相关函数以支持内核和用户页表操作
- 添加内核页表物理地址和ASID的获取与设置函数，重构相关模块
- 添加 page-table-generic 依赖，重构内存管理和页表相关功能
- 更新依赖项，重构内存地址处理，优化类型定义和对齐功能
- 重构中断处理相关函数，优化 IRQ ID 类型的使用
- 添加对无标准库环境的支持，优化相关模块配置
- 添加定时器中断确认功能，重构相关接口以支持软中断管理
- 重构系统定时器接口，支持以滴答和持续时间设置定时器，添加获取定时器频率和当前滴答计数的功能
- 重构定时器相关功能，统一命名为systimer并实现启用、禁用及设置下一个事件的功能
- 添加对中断处理的支持，重构相关逻辑并优化定时器处理
- 添加对LoongArch64架构的支持，优化中断处理和上下文切换逻辑
- 将post_allocator中的日志级别从info更改为debug，并移除内存映射设置的日志输出
- 添加byte-unit依赖并实现内存页大小功能
- integrate byte-unit crate and enhance memory management
- 添加qemu-la64配置文件，更新loongarch64构建配置，重构内存分配逻辑，优化控制台输出
- 添加内存管理和控制台功能，重构日志系统，优化模块结构
- 添加os-helper支持，重构内存管理逻辑，优化内存分配和地址转换
- 添加somehal-macros支持，重构内核和用户空间的入口逻辑，优化Cargo配置

### Fixed

- 修复中断使能函数，确保正确检查和设置特定中断

### Other

- ✨ feat(mmio-api): 更新 mmio-api 版本并修改地址类型为 MmioAddr ([#10](https://github.com/drivercraft/sparreal-os/pull/10))
- ✨ feat(mmio-api): 添加内存映射 I/O 抽象 API 以支持操作系统内核开发 ([#9](https://github.com/drivercraft/sparreal-os/pull/9))
- ♻️ refactor(chore): 清理项目配置并统一 Cargo 设置
- ✨ feat(macro): 优化 somehal::entry 宏支持参数化初始化
- ✨ feat(uspace): 添加用户空间支持，更新相关配置和实现
- ♻️ refactor(aarch64, el2): 完善 Hypervisor 模式页表与定时器支持
- 🔧 chore(somehal): 移动构建脚本并增加栈大小，添加文档
- ♻️ refactor(sparreal-rt): 移除对 someboot 的直接依赖，统一通过 somehal 访问
- ♻️ refactor(irq): 将 IRQ 处理逻辑从 someboot 迁移到 somehal
- ♻️ refactor(timer, sync): 优化定时器和自旋锁实现，移除冗余日志
- 🐛 fix(timer): 重构定时器接口，替换 systimer 为 systick，更新相关函数调用
- ♻️ refactor(platform): 重构平台层初始化流程和模块组织
- ♻️ refactor(platform): 重命名 someplat 平台层为 somehal
- ♻️ refactor(build): 重命名 somehal crate 为 someboot
- ✨ feat(drivers): 集成 rdrive 驱动框架,添加 FDT 和 PCIe 支持
- ♻️ refactor(linker): 移除冗余的驱动程序段定义，优化链接脚本
- ♻️ refactor(memory): 移除不必要的物理地址与虚拟地址转换函数，优化内存接口
- ♻️ refactor(loongarch64): 清理编译警告和 clippy 警告
- ✨ feat(pte): 更新 PteConfig 使用方式，增强页表项配置和映射逻辑
- Refactor page table entry handling to use PteConfig
- ♻️ refactor(hal): 重构地址转换方法，分离RAM和IO地址映射
- ♻️ refactor(irq): 重构中断处理架构,统一IRQ管理到somehal crate
- ✨ feat(macro): 实现中断处理器过程宏,简化IRQ处理函数编写
- ♻️ refactor(irq): 重构中断ID类型系统,统一使用IrqId替代SoftIrqId
- ♻️ refactor(mem): 改进内存映射逻辑,统一KImage和MMIO处理
- ♻️ refactor(hal): 移除PageTableOp trait,直接使用page_table_generic
- ♻️ refactor(mem): 重构内存管理接口,统一boot table管理和ioremap实现
- ♻️ refactor(aarch64): 重构页表项实现,使用tock-registers类型安全接口
- Implement basic stubs for IRQ handling and CPU ID retrieval in hal_impl.rs
- init
