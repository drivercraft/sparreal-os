# DMA API

用于 Rust 的 DMA（直接内存访问）抽象 API，提供安全的 DMA 内存操作接口，适用于嵌入式和裸机环境。

## 特性

- **`no_std` 支持**: 完全支持裸机环境
- **类型安全**: 提供类型安全的 DMA 操作接口
- **自动缓存同步**: 根据传输方向自动处理缓存刷新/失效
- **Linux 兼容语义**: API 设计遵循 Linux DMA 映射约定
- **RAII 风格**: 自动管理 DMA 映射的生命周期

## 核心类型

### `Direction` - DMA 传输方向

```rust
pub enum Direction {
    ToDevice,       // DMA_TO_DEVICE: CPU 写入，设备读取
    FromDevice,     // DMA_FROM_DEVICE: 设备写入，CPU 读取
    Bidirectional,  // DMA_BIDIRECTIONAL: 双向传输
}
```

### `DeviceDma` - DMA 设备操作

DMA 设备的主要操作接口，提供内存分配、映射和缓存操作。

### `DmaOp` trait - 操作系统抽象层

需要由平台实现的 trait，用于提供底层的 DMA 操作支持。

### `DArray<T>` - DMA 数组

DMA 可访问的数组类型，支持自动缓存同步。

### `DBox<T>` - DMA Box

DMA 可访问的单值容器，支持自动缓存同步。

### `SingleMap` - 单次映射

临时的单缓冲区 DMA 映射，RAII 风格自动清理。

## 使用示例

### 实现 `DmaOp` trait

```rust
use dma_api::{DmaOp, DmaDirection, DmaHandle, DmaError};
use core::{alloc::Layout, ptr::NonNull, num::NonZeroUsize};

struct MyDmaImpl;

impl DmaOp for MyDmaImpl {
    fn page_size(&self) -> usize {
        4096 // 返回系统页大小
    }

    unsafe fn map_single(
        &self,
        dma_mask: u64,
        addr: NonNull<u8>,
        size: NonZeroUsize,
        align: usize,
        direction: DmaDirection,
    ) -> Result<DmaHandle, DmaError> {
        // 实现虚拟地址到 DMA 地址的映射
        // 返回 DmaHandle
        todo!()
    }

    unsafe fn unmap_single(&self, handle: DmaHandle) {
        // 解除 DMA 映射
        todo!()
    }

    unsafe fn alloc_coherent(
        &self,
        dma_mask: u64,
        layout: Layout,
    ) -> Option<DmaHandle> {
        // 分配 DMA 一致性内存
        todo!()
    }

    unsafe fn dealloc_coherent(&self, handle: DmaHandle) {
        // 释放 DMA 内存
        todo!()
    }
}
```

### 使用 `DeviceDma` 和 DMA 容器

```rust,ignore
use dma_api::{DeviceDma, Direction};

// 创建 DMA 设备实例 (假设 DMA_IMPL 已实现 DmaOp trait)
static DMA_IMPL: MyDmaImpl = MyDmaImpl;
let device = DeviceDma::new(0xFFFFFFFF, &DMA_IMPL);

// 创建 DMA 数组 (自动初始化为零)
let mut dma_array = device.array_zero_with_align::<u32>(100, 64, Direction::FromDevice)
    .expect("Failed to allocate DMA array");

// 写入数据 (自动处理缓存同步)
dma_array.set(0, 0x12345678);
dma_array.set(1, 0xABCDEF00);

// 读取数据 (自动处理缓存同步)
let value = dma_array.read(0);
assert_eq!(value, Some(0x12345678));

// 获取 DMA 地址用于硬件配置
let dma_addr = dma_array.dma_addr();
// 配置 DMA 控制器使用 dma_addr...

// 使用 read 方法读取
let val = dma_array.read(0).unwrap(); // 自动处理缓存同步

// 创建 DMA Box (单个值)
#[derive(Default)]
struct MyStruct {
    field1: u32,
    field2: u32,
}

let mut dma_box = device.box_zero_with_align::<MyStruct>(64, Direction::ToDevice)
    .expect("Failed to allocate DMA box");

dma_box.write(MyStruct { field1: 42, field2: 100 });
let value = dma_box.read();

// 修改值 (read-modify-write 模式)
dma_box.modify(|v| v.field1 += 10);
```

### 使用 `SingleMap` 映射现有缓冲区

```rust,ignore
use dma_api::{DeviceDma, Direction};

// 假设 DMA_IMPL 已实现 DmaOp trait
let device = DeviceDma::new(0xFFFFFFFF, &DMA_IMPL);

// 现有缓冲区
let mut buffer = [0u8; 4096];

// 映射缓冲区用于 DMA
let mapping = device.map_single(&buffer, 64, Direction::ToDevice)
    .expect("Mapping failed");

// 使用前需要手动同步缓存
mapping.prepare_read_all();   // 如果需要从设备读取
// ... DMA 传输 ...
mapping.confirm_write_all();  // 如果需要写入设备

// 使用映射的 DMA 地址
let dma_addr = mapping.dma_addr();

// 映射在离开作用域时自动解除（不会自动同步缓存）
```

## 缓存同步

API 遵循 Linux DMA 缓存一致性语义，由 `DmaOp` trait 的 `prepare_read` 和 `confirm_write` 方法自动处理：

### 读操作前 (`prepare_read`)

- **FromDevice/Bidirectional**: 执行 `invalidate` - 使 CPU 缓存失效，准备接收设备数据
- **ToDevice**: 无需操作

### 写操作后 (`confirm_write`)

- **ToDevice/Bidirectional**: 执行 `flush` - 将 CPU 数据写回内存
- **FromDevice**: 无需操作

### DMA 容器的自动同步

`DArray<T>` 和 `DBox<T>` 在以下操作时自动处理缓存同步：

- `read()` / `set()` - 自动同步对应元素
- `write()` / `modify()` - 自动同步写入
- `copy_from_slice()` - 写入后自动同步整个范围

### SingleMap 的手动同步

**重要**：`SingleMap` **不会**在 Drop 时自动同步缓存，用户必须显式调用缓存同步方法：

```rust,ignore
let mapping = device.map_single(&buffer, 64, Direction::ToDevice)?;

// 写入前准备
mapping.confirm_write_all();  // 将 CPU 数据刷到内存

// ... DMA 传输 ...

// 读取前准备
mapping.prepare_read_all();   // 使 CPU 缓存失效

// Drop 时只会解除映射，不会自动同步
```

这与 Linux DMA API 的行为一致，让用户可以精确控制缓存同步的时机。

## API 参考

### 核心类型

| 类型            | 说明                 |
| :-------------- | :------------------- |
| `DeviceDma`     | DMA 设备操作接口     |
| `Direction`     | DMA 传输方向枚举     |
| `DmaOp`         | 操作系统抽象层 trait |
| `DmaHandle`     | DMA 映射句柄         |
| `DArray<T>`     | DMA 数组容器         |
| `DBox<T>`       | DMA 单值容器         |
| `SingleMap`     | 单次映射             |

### Linux 等价 API

| Rust API                    | Linux Equivalent                     |
| :-------------------------- | :----------------------------------- |
| `DeviceDma::map_single()`   | `dma_map_single()`                   |
| `SingleMap::drop()`         | `dma_unmap_single()`                 |
| `DmaOp::alloc_coherent()`   | `dma_alloc_coherent()`               |
| `DmaOp::dealloc_coherent()` | `dma_free_coherent()`                |
| `DmaOp::flush()`            | `dma_cache_sync()` (DMA_TO_DEVICE)   |
| `DmaOp::invalidate()`       | `dma_cache_sync()` (DMA_FROM_DEVICE) |

### 对齐要求

DMA 操作通常需要对齐到特定的边界：

- `array_zero_with_align()` / `box_zero_with_align()` 的 `align` 参数指定对齐字节数
- 常见对齐值：64、128、256、512、4096
- 确保返回的 DMA 地址满足对齐要求

### DMA Mask

`DeviceDma::new()` 的 `dma_mask` 参数指定设备可寻址的地址范围：

- `0xFFFFFFFF` (32 位设备)
- `0xFFFFFFFFFFFFFFFF` (64 位设备)
- 其他值根据设备硬件限制
