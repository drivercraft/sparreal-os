use axplat::mem::{MemIf, PhysAddr, RawRange, VirtAddr};

struct MemIfImpl;

#[impl_plat_interface]
impl MemIf for MemIfImpl {
    fn phys_ram_ranges() -> &'static [RawRange] {
        todo!()
    }

    fn reserved_phys_ram_ranges() -> &'static [RawRange] {
        todo!()
    }

    fn mmio_ranges() -> &'static [RawRange] {
        &[]
    }

    fn phys_to_virt(paddr: PhysAddr) -> VirtAddr {
        (somehal::mem::phys_to_virt(paddr.as_usize()) as usize).into()
    }

    fn virt_to_phys(vaddr: VirtAddr) -> PhysAddr {
        somehal::mem::virt_to_phys(vaddr.as_ptr()).into()
    }

    fn kernel_aspace() -> (VirtAddr, usize) {
        todo!()
    }
}
