use alloc::boxed::Box;
use core::ptr::NonNull;

use somehal::{MemConfig, irq_handler, mem::PteConfig};
use sparreal_kernel::{
    hal::al::{AccessFlags, *},
    impl_trait,
    os::mem::KernelAllocator,
};

struct InitImpl;

impl_trait! {
impl Platform for InitImpl {

    fn shutdown() -> ! {
        somehal::power::shutdown()
    }
    fn irq_is_enabled(irq: IrqId) -> bool {
        somehal::irq::irq_is_enabled(irq.raw().into())
    }
    fn irq_set_enabled(irq: IrqId, enable: bool) {
        somehal::irq::irq_set_enable(irq.raw().into(), enable);
    }

    fn fdt_addr() -> Option<NonNull<u8>> {
        somehal::fdt_addr().map(|ptr| unsafe{ NonNull::new_unchecked(ptr)})
    }

    fn post_paging() {
        somehal::post_paging();
    }
}
}

struct MemoryImpl;

impl_trait! {
impl Memory for MemoryImpl {
    fn _va(paddr: PhysAddr) -> VirtAddr {
        somehal::mem::__va(paddr.raw() as _).into()
    }
    fn _io(paddr: PhysAddr) -> VirtAddr {
        somehal::mem::__io(paddr.raw() as _).into()
    }
    fn _percpu(paddr: PhysAddr) -> VirtAddr {
        somehal::mem::__percpu(paddr.raw() as _).into()
    }

    fn kimage_offset() -> isize {
        somehal::mem::vm_load_offset()
    }

    fn virt_to_phys(virt: VirtAddr) -> PhysAddr {
        somehal::mem::virt_to_phys(virt.raw() as _).into()
    }

    fn page_size() -> usize {
        somehal::mem::page_size()
    }

    fn memory_map() -> &'static[ MemoryDescriptor] {
        somehal::mem::memory_map()
    }

    fn page_table_new() -> Result< Box<dyn PageTable>, PagingError> {
        Ok(Box::new( PageTableImpl( somehal::mem::mmu::new_page_table(KernelAllocator)?)))
    }

    fn kernel_page_table() -> PhysAddr {
        let paddr = somehal::kernel_page_table_paddr();
        PhysAddr::new(paddr)
    }

    fn set_kernel_page_table(pt: PhysAddr) {
        somehal::set_kernel_page_table_paddr(pt.raw());
    }

    fn user_page_table() -> PageTableInfo {

        #[cfg(uspace)]
        {
            somehal::user_page_table()
        }
        #[cfg(hv)]
        {
            PageTableInfo { asid: 0, addr: 0 }
        }
    }

    fn set_user_page_table(_pt: PageTableInfo) {
        #[cfg(uspace)]
        somehal::set_user_page_table(_pt);
    }
}
}

pub struct PageTableImpl(somehal::mem::mmu::ArchPageTable<KernelAllocator>);

impl PageTable for PageTableImpl {
    fn addr(&self) -> PhysAddr {
        PhysAddr::new(self.0.root_paddr().raw())
    }

    fn map(
        &mut self,
        virt_start: VirtAddr,
        phys_start: PhysAddr,
        size: usize,
        settings: MemConfig,
        flush: bool,
    ) -> Result<(), PagingError> {
        let pte = PteConfig {
            valid: true,
            read: true,
            writable: settings.access.contains(AccessFlags::WRITE),
            executable: settings.access.contains(AccessFlags::EXECUTE),
            mem_attr: settings.attrs,
            ..Default::default()
        };

        self.0.map(&somehal::mem::MapConfig {
            vaddr: virt_start.raw().into(),
            paddr: phys_start.raw().into(),
            size,
            pte,
            allow_huge: true,
            flush,
        })
    }

    fn unmap(&mut self, virt_start: VirtAddr, size: usize) -> Result<(), PagingError> {
        self.0.unmap(virt_start.raw().into(), size)
    }
}

struct CpuImpl;

impl_trait! {
impl Cpu for CpuImpl {
    fn cpu_count() -> usize {
        somehal::smp::cpu_count()
    }

    fn cpu_on(cpu_idx: usize) -> usize {
        match somehal::power::cpu_on(cpu_idx) {
            Ok(()) => 0,
            Err(somehal::power::CpuOnError::NotSupported) => 1,
            Err(somehal::power::CpuOnError::AlreadyOn) => 2,
            Err(somehal::power::CpuOnError::InvalidParameters) => 3,
            Err(somehal::power::CpuOnError::Other(_)) => 4,
        }
    }

    fn current_cpu_id() -> usize {
        somehal::smp::cpu_idx()
    }

    fn irq_local_is_enabled() -> bool {
        somehal::irq::irq_local_is_enabled()
    }

    fn irq_local_set_enable(enable: bool) {
        somehal::irq::irq_local_set_enable(enable);
    }

    fn systick_irq_id() -> IrqId {
       let irq: usize = somehal::irq::systick_irq().into();
         IrqId::from(irq)
    }

    fn systick_enable() {
        somehal::timer::enable();
    }

    fn systick_irq_enable() {
        somehal::timer::irq_enable();
    }

    fn systick_irq_disable() {
        somehal::timer::irq_disable();
    }

    fn systick_irq_is_enabled() -> bool {
        somehal::timer::irq_is_enabled()
    }

    fn systick_ack() {
        somehal::timer::ack();
    }

    fn systick_frequency() -> usize {
        somehal::timer::freq()
    }

    fn systick_ticks() -> usize {
        somehal::timer::ticks()
    }

    fn systick_set_interval(ticks: usize){
        somehal::timer::set_next_event_in_ticks(ticks);
    }

}
}

struct ConsoleImpl;

impl_trait! {
impl Console for ConsoleImpl {
    fn early_write(bytes: &[u8]) -> usize {
        somehal::console::_write_bytes(bytes)
    }

    fn early_read() -> Option<u8> {
        None
    }
}
}

#[irq_handler]
fn somehal_handle_irq(irq: somehal::irq::IrqId) {
    handle_irq(irq.raw().into());
}
