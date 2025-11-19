use heapless::Vec;

#[derive(Debug, Clone, Copy)]
pub struct MemoryDescriptor {
    // pub name: &'static str,
    pub physical_start: usize,
    pub size_in_bytes: usize,
    pub memory_type: MemoryType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryType {
    Usable,
    Reserved,
}

/// 收集所有内存区域的边界点并排序去重
fn collect_boundaries(
    ram: &[MemoryDescriptor],
    rsv: &[MemoryDescriptor],
) -> heapless::Vec<usize, 64> {
    let mut boundaries = heapless::Vec::<usize, 64>::new();

    // 收集RAM区域的边界点
    for region in ram {
        let _ = boundaries.push(region.physical_start);
        let _ = boundaries.push(region.physical_start + region.size_in_bytes);
    }

    // 收集RSV区域的边界点
    for region in rsv {
        let _ = boundaries.push(region.physical_start);
        let _ = boundaries.push(region.physical_start + region.size_in_bytes);
    }

    // 手动排序（heapless::Vec在no_std环境下没有sort方法）
    for i in 0..boundaries.len() {
        for j in i + 1..boundaries.len() {
            if boundaries[i] > boundaries[j] {
                boundaries.swap(i, j);
            }
        }
    }

    // 手动去重（heapless::Vec没有dedup方法）
    let mut unique_boundaries = heapless::Vec::<usize, 64>::new();
    for &boundary in &boundaries {
        if unique_boundaries.last() != Some(&boundary) {
            let _ = unique_boundaries.push(boundary);
        }
    }

    unique_boundaries
}

/// 基于边界点生成连续的内存区间段
fn generate_segments(boundaries: &[usize]) -> heapless::Vec<(usize, usize), 128> {
    let mut segments = heapless::Vec::<(usize, usize), 128>::new();

    if boundaries.len() < 2 {
        return segments;
    }

    // 生成连续的区间段
    for i in 0..boundaries.len() - 1 {
        let start = boundaries[i];
        let end = boundaries[i + 1];

        if start < end {
            let _ = segments.push((start, end));
        }
    }

    segments
}

/// 判断区间段是否与任何保留区域相交
fn determine_segment_type(
    segment_start: usize,
    segment_end: usize,
    rsv: &[MemoryDescriptor],
) -> MemoryType {
    for reserved in rsv {
        let rsv_start = reserved.physical_start;
        let rsv_end = reserved.physical_start + reserved.size_in_bytes;

        // 检查是否相交
        if segment_start < rsv_end && segment_end > rsv_start {
            return MemoryType::Reserved;
        }
    }

    MemoryType::Usable
}

/// 对保留区域进行页面对齐扩展
fn align_reserved_regions(
    rsv: &[MemoryDescriptor],
    page_size: usize,
) -> heapless::Vec<MemoryDescriptor, 64> {
    let mut aligned_rsv = heapless::Vec::<MemoryDescriptor, 64>::new();

    for reserved in rsv {
        // 向前对齐start地址
        let aligned_start = (reserved.physical_start / page_size) * page_size;

        // 向后对齐end地址
        let end = reserved.physical_start + reserved.size_in_bytes;
        let aligned_end = end.div_ceil(page_size) * page_size;

        let aligned_size = aligned_end - aligned_start;

        if aligned_size > 0 {
            let aligned_descriptor = MemoryDescriptor {
                physical_start: aligned_start,
                size_in_bytes: aligned_size,
                memory_type: reserved.memory_type,
            };

            if aligned_rsv.push(aligned_descriptor).is_err() {
                break;
            }
        }
    }

    aligned_rsv
}

/// 合并连续的同类型内存描述符
fn merge_consecutive_segments(segments: Vec<MemoryDescriptor, 64>) -> Vec<MemoryDescriptor, 64> {
    if segments.is_empty() {
        return segments;
    }

    let mut merged = Vec::<MemoryDescriptor, 64>::new();

    // 按起始地址手动排序（heapless::Vec在no_std环境下没有sort方法）
    let mut sorted_segments = segments;
    for i in 0..sorted_segments.len() {
        for j in i + 1..sorted_segments.len() {
            if sorted_segments[i].physical_start > sorted_segments[j].physical_start {
                sorted_segments.swap(i, j);
            }
        }
    }

    let mut current = sorted_segments[0];

    for segment in sorted_segments.iter().skip(1) {
        // 检查是否连续且类型相同
        let current_end = current.physical_start + current.size_in_bytes;

        if current_end == segment.physical_start && current.memory_type == segment.memory_type {
            // 合并到当前段
            current.size_in_bytes += segment.size_in_bytes;
        } else {
            // 保存当前段，开始新段
            let _ = merged.push(current);
            current = *segment;
        }
    }

    // 添加最后一段
    let _ = merged.push(current);

    merged
}

pub fn merge_memories(
    ram: &[MemoryDescriptor],
    rsv: &[MemoryDescriptor],
    page_size: usize,
) -> Vec<MemoryDescriptor, 64> {
    let mut result = Vec::<MemoryDescriptor, 64>::new();

    // 1. 对保留区域进行页面对齐扩展
    let aligned_rsv = align_reserved_regions(rsv, page_size);

    // 2. 收集所有边界点
    let boundaries = collect_boundaries(ram, &aligned_rsv);

    // 3. 生成连续区间段
    let segments = generate_segments(&boundaries);

    // 4. 为每个区间段确定类型并创建内存描述符
    for (segment_start, segment_end) in segments {
        let memory_type = determine_segment_type(segment_start, segment_end, &aligned_rsv);

        let descriptor = MemoryDescriptor {
            physical_start: segment_start,
            size_in_bytes: segment_end - segment_start,
            memory_type,
        };

        if result.push(descriptor).is_err() {
            break;
        }
    }

    // 5. 合并连续的同类型区间
    merge_consecutive_segments(result)
}

#[cfg(all(not(target_os = "none"), test))]
mod test {
    extern crate std;
    use super::*;
    use std::println;
    use std::vec;
    use std::vec::Vec as StdVec;

    #[test]
    fn test_merge_memories() {
        const PAGE_SIZE: usize = 4096;

        // 测试1：RAM被RSV分割
        {
            let ram: StdVec<MemoryDescriptor> = vec![MemoryDescriptor {
                physical_start: 0x1000,
                size_in_bytes: 0x4000,
                memory_type: MemoryType::Usable,
            }];

            let rsv: StdVec<MemoryDescriptor> = vec![MemoryDescriptor {
                physical_start: 0x2000,
                size_in_bytes: 0x1000,
                memory_type: MemoryType::Reserved,
            }];

            let result = merge_memories(&ram, &rsv, PAGE_SIZE);

            // 预期结果：3个区间 - [0x1000-0x2000]可用, [0x2000-0x3000]保留, [0x3000-0x5000]可用
            assert_eq!(result.len(), 3);

            // 第一个可用区间：0x1000-0x2000
            assert_eq!(result[0].physical_start, 0x1000);
            assert_eq!(result[0].size_in_bytes, 0x1000);
            assert_eq!(result[0].memory_type, MemoryType::Usable);

            // 保留区间：0x2000-0x3000
            assert_eq!(result[1].physical_start, 0x2000);
            assert_eq!(result[1].size_in_bytes, 0x1000);
            assert_eq!(result[1].memory_type, MemoryType::Reserved);

            // 第二个可用区间：0x3000-0x5000
            assert_eq!(result[2].physical_start, 0x3000);
            assert_eq!(result[2].size_in_bytes, 0x2000);
            assert_eq!(result[2].memory_type, MemoryType::Usable);
        }

        // 测试2：无相交的情况
        {
            let ram: StdVec<MemoryDescriptor> = vec![
                MemoryDescriptor {
                    physical_start: 0x1000,
                    size_in_bytes: 0x1000,
                    memory_type: MemoryType::Usable,
                },
                MemoryDescriptor {
                    physical_start: 0x5000,
                    size_in_bytes: 0x1000,
                    memory_type: MemoryType::Usable,
                },
            ];

            let rsv: StdVec<MemoryDescriptor> = vec![MemoryDescriptor {
                physical_start: 0x3000,
                size_in_bytes: 0x1000,
                memory_type: MemoryType::Reserved,
            }];

            let result = merge_memories(&ram, &rsv, PAGE_SIZE);

            // 预期结果：3个区间，保持原样
            assert_eq!(result.len(), 3);
            assert_eq!(result[0].memory_type, MemoryType::Usable);
            assert_eq!(result[1].memory_type, MemoryType::Reserved);
            assert_eq!(result[2].memory_type, MemoryType::Usable);
        }

        // 测试3：完全包含的情况
        {
            let ram: StdVec<MemoryDescriptor> = vec![MemoryDescriptor {
                physical_start: 0x2000,
                size_in_bytes: 0x4000,
                memory_type: MemoryType::Usable,
            }];

            let rsv: StdVec<MemoryDescriptor> = vec![MemoryDescriptor {
                physical_start: 0x3000,
                size_in_bytes: 0x1000,
                memory_type: MemoryType::Reserved,
            }];

            let result = merge_memories(&ram, &rsv, PAGE_SIZE);

            // 预期结果：RSV在页面对齐后会覆盖RAM的一部分
            assert!(result.len() >= 1);
        }

        // 测试4：空输入
        {
            let ram: StdVec<MemoryDescriptor> = vec![];
            let rsv: StdVec<MemoryDescriptor> = vec![];

            let result = merge_memories(&ram, &rsv, PAGE_SIZE);
            assert_eq!(result.len(), 0);
        }

        // 测试5：只有RAM，无RSV
        {
            let ram: StdVec<MemoryDescriptor> = vec![MemoryDescriptor {
                physical_start: 0x1000,
                size_in_bytes: 0x2000,
                memory_type: MemoryType::Usable,
            }];
            let rsv: StdVec<MemoryDescriptor> = vec![];

            let result = merge_memories(&ram, &rsv, PAGE_SIZE);
            assert_eq!(result.len(), 1);
            assert_eq!(result[0].memory_type, MemoryType::Usable);
            assert_eq!(result[0].physical_start, 0x1000);
            assert_eq!(result[0].size_in_bytes, 0x2000);
        }

        // 测试6：只有RSV，无RAM
        {
            let ram: StdVec<MemoryDescriptor> = vec![];
            let rsv: StdVec<MemoryDescriptor> = vec![MemoryDescriptor {
                physical_start: 0x2000,
                size_in_bytes: 0x1000,
                memory_type: MemoryType::Reserved,
            }];

            let result = merge_memories(&ram, &rsv, PAGE_SIZE);
            // 页面对齐后RSV会被扩展
            assert_eq!(result.len(), 1);
            assert_eq!(result[0].memory_type, MemoryType::Reserved);
        }

        // 测试7：连续的同类型区域合并
        {
            let ram: StdVec<MemoryDescriptor> = vec![
                MemoryDescriptor {
                    physical_start: 0x1000,
                    size_in_bytes: 0x1000,
                    memory_type: MemoryType::Usable,
                },
                MemoryDescriptor {
                    physical_start: 0x2000,
                    size_in_bytes: 0x1000,
                    memory_type: MemoryType::Usable,
                },
            ];
            let rsv: StdVec<MemoryDescriptor> = vec![];

            let result = merge_memories(&ram, &rsv, PAGE_SIZE);
            // 应该合并为一个区间
            assert_eq!(result.len(), 1);
            assert_eq!(result[0].physical_start, 0x1000);
            assert_eq!(result[0].size_in_bytes, 0x2000);
            assert_eq!(result[0].memory_type, MemoryType::Usable);
        }

        // 测试8：页面对齐边界情况
        {
            let ram: StdVec<MemoryDescriptor> = vec![MemoryDescriptor {
                physical_start: 0x1000,
                size_in_bytes: 0x2000,
                memory_type: MemoryType::Usable,
            }];

            // RSV区域刚好在页面边界
            let rsv: StdVec<MemoryDescriptor> = vec![MemoryDescriptor {
                physical_start: 0x2000,
                size_in_bytes: 0x100,
                memory_type: MemoryType::Reserved,
            }];

            let result = merge_memories(&ram, &rsv, PAGE_SIZE);
            // 由于页面对齐，RSV会扩展到整个页面
            assert!(result.len() >= 2);

            // 检查是否有保留区域
            let has_reserved = result
                .iter()
                .any(|desc| desc.memory_type == MemoryType::Reserved);
            assert!(has_reserved);
        }
    }
}
