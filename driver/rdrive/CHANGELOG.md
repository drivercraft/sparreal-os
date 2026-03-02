# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.18.11](https://github.com/drivercraft/rdrive/compare/rdrive-v0.18.10...rdrive-v0.18.11) - 2025-10-16

### Other

- 更新时钟驱动实现，替换 UART 驱动并调整探测函数
- serial

## [0.18.7](https://github.com/drivercraft/rdrive/compare/rdrive-v0.18.4...rdrive-v0.18.7) - 2025-09-25

### Fixed

- remove unused dependency enum_dispatch from Cargo.toml

## [0.18.4](https://github.com/drivercraft/rdrive/compare/rdrive-v0.18.3...rdrive-v0.18.4) - 2025-09-25

### Other

- add pcie

## [0.16.0](https://github.com/drivercraft/rdrive/compare/rdrive-v0.15.2...rdrive-v0.16.0) - 2025-06-27

### Added

- 添加 fdt_phandle_to_device_id 方法并在示例中使用

### Other

- 简化 fdt_phandle_to_device_id 函数中的模式匹配
- Merge branch 'main' of github.com:drivercraft/rdrive

## [0.15.2](https://github.com/drivercraft/rdrive/compare/rdrive-v0.15.1...rdrive-v0.15.2) - 2025-06-26

### Other

- 修改 force_use 方法，简化返回值类型
- Merge branch 'main' of github.com:drivercraft/rdrive
- 更新示例链接，指向 GitHub 上的具体实现

## [0.15.1](https://github.com/drivercraft/rdrive/compare/rdrive-v0.15.0...rdrive-v0.15.1) - 2025-06-26

### Other

- 更新 README.md，添加架构概述和驱动注册示例

## [0.15.0](https://github.com/drivercraft/rdrive/compare/rdrive-v0.14.3...rdrive-v0.15.0) - 2025-06-25

### Added

- add OnProbeError type and refactor probe functions to use it

### Other

- Merge branch 'main' of github.com:drivercraft/rdrive

## [0.14.3](https://github.com/drivercraft/rdrive/compare/rdrive-v0.14.2...rdrive-v0.14.3) - 2025-06-25

### Added

- implement Send and Sync traits for Device struct

### Other

- simplify device locking and retrieval methods

## [0.14.2](https://github.com/drivercraft/rdrive/compare/rdrive-v0.14.1...rdrive-v0.14.2) - 2025-06-25

### Added

- add rdif-net package and implement Interface trait

### Other

- Merge branch 'main' of github.com:drivercraft/rdrive

## [0.14.1](https://github.com/drivercraft/rdrive/compare/rdrive-v0.14.0...rdrive-v0.14.1) - 2025-06-24

### Other

- update driver macros to use AsAny for type downcasting
