use crate::{
    FramAllocator, PagingError, PagingResult, TableGeneric, VirtAddr,
    map::{Frame, MapConfig, MapRecursiveConfig},
    walk::{PageTableWalker, WalkConfig},
};

/// 页表结构
pub struct PageTable<T: TableGeneric, A: FramAllocator> {
    pub root: Frame<T, A>,
}

impl<T: TableGeneric, A: FramAllocator> core::fmt::Debug for PageTable<T, A>
where
    T::P: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PageTable")
            .field("root_paddr", &format_args!("{:#x}", self.root.paddr.raw()))
            .field("table_levels", &T::LEVEL)
            .field("max_block_level", &T::MAX_BLOCK_LEVEL)
            .field("page_size", &format_args!("{:#x}", T::PAGE_SIZE))
            .finish()
    }
}

impl<T: TableGeneric, A: FramAllocator> PageTable<T, A> {
    /// 创建一个新的页表
    pub fn new(allocator: A) -> PagingResult<Self> {
        let root = Frame::new(allocator)?;
        Ok(Self { root })
    }

    /// 映射虚拟地址范围到物理地址范围
    pub fn map(&mut self, config: &MapConfig<T::P>) -> PagingResult {
        // 验证输入参数
        self.validate_map_config(config)?;

        // 检查大小溢出
        if config.vaddr.raw().checked_add(config.size).is_none()
            || config.paddr.raw().checked_add(config.size).is_none()
        {
            return Err(PagingError::address_overflow(
                "Virtual or physical address overflow",
            ));
        }

        self.root.map_range_recursive(MapRecursiveConfig {
            start_vaddr: config.vaddr,
            start_paddr: config.paddr,
            end_vaddr: config.vaddr + config.size,
            level: T::LEVEL,
            allow_huge: config.allow_huge,
            flush: config.flush,
            pte_template: config.pte,
        })?;

        Ok(())
    }

    /// 创建页表遍历迭代器
    pub fn walk(&self, config: WalkConfig) -> PageTableWalker<T, A> {
        PageTableWalker::new(self, config)
    }

    /// 遍历所有有效页表项
    pub fn walk_valid(&self) -> PageTableWalker<T, A> {
        let config = WalkConfig {
            start_vaddr: VirtAddr::new(0),
            end_vaddr: VirtAddr::new(usize::MAX),
            visit_invalid: false,
        };
        self.walk(config)
    }

    /// 验证映射配置的有效性
    fn validate_map_config(&self, config: &MapConfig<T::P>) -> PagingResult {
        if config.size == 0 {
            return Err(PagingError::invalid_size("Size cannot be zero"));
        }

        // 检查虚拟地址和物理地址是否页对齐
        if config.vaddr.raw() % T::PAGE_SIZE != 0 {
            return Err(PagingError::alignment_error(
                "Virtual address not page aligned",
            ));
        }

        if config.paddr.raw() % T::PAGE_SIZE != 0 {
            return Err(PagingError::alignment_error(
                "Physical address not page aligned",
            ));
        }

        Ok(())
    }
}
