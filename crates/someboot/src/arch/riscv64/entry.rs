use core::{
    arch::naked_asm,
    mem::offset_of,
    sync::atomic::{AtomicUsize, Ordering},
};

use crate::{entry::PrimaryCpuInitInfo, smp::PerCpuMeta};

const MAX_COLD_BOOT_HARTS: usize = 64;
const BOOT_STATE_UNCLAIMED: usize = usize::MAX;
const BOOT_STATE_PRIMARY_INIT: usize = usize::MAX - 1;
const BOOT_STATE_SECONDARY_READY: usize = usize::MAX - 2;

static BOOT_STATE: AtomicUsize = AtomicUsize::new(BOOT_STATE_UNCLAIMED);
static SECONDARY_BOOT_META: [AtomicUsize; MAX_COLD_BOOT_HARTS] =
    [const { AtomicUsize::new(0) }; MAX_COLD_BOOT_HARTS];
static SECONDARY_BOOT_RELEASE: [AtomicUsize; MAX_COLD_BOOT_HARTS] =
    [const { AtomicUsize::new(0) }; MAX_COLD_BOOT_HARTS];

#[unsafe(naked)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _head() -> ! {
    naked_asm!(
        "mv t0, a0",
        "mv t1, a1",
        "mv tp, t0",
        "lla t2, {boot_state}",
        "ld t5, 0(t2)",
        "li t3, {boot_state_unclaimed}",
        "li t4, {boot_state_primary_init}",
        "bne t5, t3, 2f",
        "sd t4, 0(t2)",
        "lla sp, __cpu0_stack_top",
        "mv a0, t0",
        "mv a1, t1",
        "j {primary_head_entry}",
        "2:",
        "li t3, {max_cold_boot_harts}",
        "bgeu t0, t3, 5f",
        "li t3, {boot_state_secondary_ready}",
        "3:",
        "ld t4, 0(t2)",
        "bne t4, t3, 3b",
        "slli t3, t0, 3",
        "lla t4, {secondary_meta}",
        "add t4, t4, t3",
        "4:",
        "ld t5, 0(t4)",
        "beqz t5, 4b",
        "lla t6, {secondary_release}",
        "add t6, t6, t3",
        "6:",
        "ld a0, 0(t6)",
        "beqz a0, 6b",
        "fence r, rw",
        "mv a0, t5",
        "ld sp, {stack_top_offset}(t5)",
        "j {secondary_start}",
        "5:",
        "j 5b",
        boot_state = sym BOOT_STATE,
        boot_state_unclaimed = const BOOT_STATE_UNCLAIMED,
        boot_state_primary_init = const BOOT_STATE_PRIMARY_INIT,
        boot_state_secondary_ready = const BOOT_STATE_SECONDARY_READY,
        max_cold_boot_harts = const MAX_COLD_BOOT_HARTS,
        secondary_meta = sym SECONDARY_BOOT_META,
        secondary_release = sym SECONDARY_BOOT_RELEASE,
        stack_top_offset = const offset_of!(PerCpuMeta, stack_top),
        secondary_start = sym secondary_start,
        primary_head_entry = sym primary_head_entry,
    )
}

fn primary_head_entry(_hart_id: usize, fdt_addr: usize) -> ! {
    super::relocate::apply();
    primary_entry(fdt_addr)
}

fn primary_entry(fdt_addr: usize) -> ! {
    clear_bss();
    unsafe {
        crate::fdt::FDT_ADDR = fdt_addr;
    }

    <<super::Arch as crate::ArchTrait>::Console as crate::console::ArchConsoleOps>::init();
    println!("RISC-V64 SBI kernel entry.");

    let kernel_start = super::kernel_load_address();

    crate::entry::primary_init_early(PrimaryCpuInitInfo {
        kernel_start: kernel_start.into(),
        kernel_end: (__kernel_code_end as *const () as usize).into(),
        kernel_start_link: crate::consts::VM_LOAD_ADDRESS.into(),
    });
    prepare_secondary_boot();
    BOOT_STATE.store(BOOT_STATE_SECONDARY_READY, Ordering::Release);
    super::paging::enable_mmu()
}

pub(crate) fn mmu_entry() -> ! {
    super::relocate::reset();
    super::trap::setup();
    crate::prime_entry()
}

unsafe extern "C" {
    fn __kernel_code_end();
    fn __bss_start();
    fn __bss_stop();
}

#[unsafe(naked)]
pub(crate) unsafe extern "C" fn _secondary_entry(_hartid: usize, _cpu_meta_paddr: usize) -> ! {
    naked_asm!(
        "mv tp, a0",
        "mv t0, a1",
        "ld sp, {stack_top_offset}(t0)",
        "mv a0, t0",
        "j {secondary_start}",
        secondary_start = sym secondary_start,
        stack_top_offset = const offset_of!(PerCpuMeta, stack_top),
    )
}

fn secondary_start(cpu_meta_paddr: usize) -> ! {
    super::paging::enable_mmu_secondary(cpu_meta_paddr)
}

fn clear_bss() {
    let start = __bss_start as *const () as usize;
    let end = __bss_stop as *const () as usize;
    let len = end.saturating_sub(start);
    if len != 0 {
        unsafe {
            core::ptr::write_bytes(start as *mut u8, 0, len);
        }
    }
}

pub(crate) fn release_secondary_hart(hart_id: usize) -> Result<(), ColdBootReleaseError> {
    if hart_id >= MAX_COLD_BOOT_HARTS {
        return Err(ColdBootReleaseError::InvalidHartId);
    }
    if SECONDARY_BOOT_META[hart_id].load(Ordering::Acquire) == 0 {
        return Err(ColdBootReleaseError::NotPrepared);
    }
    match SECONDARY_BOOT_RELEASE[hart_id].compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
    {
        Ok(_) => Ok(()),
        Err(_) => Err(ColdBootReleaseError::AlreadyReleased),
    }
}

fn prepare_secondary_boot() {
    for cpu_idx in 0..crate::smp::cpu_count() {
        let hart_id = crate::smp::cpu_idx_to_id(cpu_idx).expect("missing hart id for cpu index");
        assert!(
            hart_id < MAX_COLD_BOOT_HARTS,
            "hart id {hart_id:#x} exceeds cold boot table size"
        );
        let meta_paddr = crate::smp::cpu_meta_addr(cpu_idx).expect("missing cpu meta address");
        SECONDARY_BOOT_META[hart_id].store(meta_paddr, Ordering::Release);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ColdBootReleaseError {
    InvalidHartId,
    NotPrepared,
    AlreadyReleased,
}
