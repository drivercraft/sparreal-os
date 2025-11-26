use core::{arch::naked_asm, mem::offset_of, ops::Deref};

use loongArch64::register::{ecfg, eentry, estat, tlbrentry};

use crate::{
    arch::{
        cache::local_flush_icache_range,
        context::TrapFrame,
        register::{csr, irq::TI},
    },
    mem::StaticCell,
};

const VECSIZE: usize = 0x200;

#[repr(C)]
#[derive(Clone, Copy)]
struct Vector([u8; VECSIZE]);

// 等效于 C: long exception_handlers[VECSIZE * 128 / sizeof(long)] __aligned(SZ_64K);
// 在 64 位系统中，sizeof(long) = 8，所以数组大小为 VECSIZE * 128 / 8 = VECSIZE * 16
#[repr(C, align(65536))] // 65536 = 64KB 对齐
struct ExceptionHandlers([Vector; 128]);

impl ExceptionHandlers {
    const fn new() -> Self {
        Self([Vector([0; VECSIZE]); 128])
    }
}

static EXCEPTION_HANDLERS: StaticCell<ExceptionHandlers> =
    StaticCell::new(Some(ExceptionHandlers::new()));

fn eentry_addr() -> usize {
    EXCEPTION_HANDLERS.0.as_ptr() as usize
}

fn tlbrentry_addr() -> usize {
    eentry_addr() + 80 * VECSIZE
}

pub fn per_cpu_trap_init(is_primary: bool) {
    setup_vint_size();
    configure_exception_vector();

    if is_primary {
        for i in 0..64 {
            set_handler(i, handle_reserved);
        }
        for i in 64..=64 + 14 {
            set_handler(i, handle_vint);
        }

        local_flush_icache_range(eentry_addr(), eentry_addr() + 0x400);
    }
}

fn setup_vint_size() {
    let n = (VECSIZE / 4).ilog2();
    ecfg::set_vs(n as _);
}

/// 配置异常向量
/// 等效于 C 的 configure_exception_vector()
fn configure_exception_vector() {
    eentry::set_eentry(eentry_addr());
    tlbrentry::set_tlbrentry(tlbrentry_addr());
}

fn set_handler(idx: usize, handler: unsafe extern "C" fn()) {
    unsafe {
        let src = core::slice::from_raw_parts(handler as *const u8, VECSIZE);
        EXCEPTION_HANDLERS.update(|vec| {
            let dst = &mut vec.0[idx].0[..];
            dst.copy_from_slice(src);

            local_flush_icache_range(
                dst.as_ptr_range().start as usize,
                dst.as_ptr_range().end as usize,
            );
        });
    }
}

unsafe extern "C" fn handle_reserved() {}

#[unsafe(naked)]
unsafe extern "C" fn handle_vint() {
    naked_asm!(
        backup_t0t1!(),
        "move    $t0,  $sp",
        "addi.d  $sp,  $sp, -{frame_size}",
        push_general_regs!(),
        "st.d    $t0, $sp, {tf_sp}",
        restore_t0t1!(),
        "st.d    $t0, $sp, {tf_t0}",
        "st.d    $t1, $sp, {tf_t1}",
        "csrrd   $t0, {prmd}",
        "st.d    $t0, $sp, {tf_prmd}",
        "csrrd   $t0, {era}",
        "st.d    $t0, $sp, {tf_era}",
        "move    $t0,  $sp",
        "bl {do_vint}",
        "ld.d    $t0,  $sp, {tf_era}",
        "csrwr    $t0, {era}",
        "ld.d    $t0,  $sp, {tf_prmd}",
        "csrwr    $t0, {prmd}",
        pop_general_regs!(),
        "ld.d    $sp,  $sp, {tf_sp}",
        "ertn",
        do_vint = sym do_vint,
        frame_size = const size_of::<TrapFrame>(),
        tf_sp = const offset_of!(TrapFrame, regs.sp),
        tf_t0 = const offset_of!(TrapFrame, regs.t0),
        tf_t1 = const offset_of!(TrapFrame, regs.t1),
        tf_prmd = const offset_of!(TrapFrame, prmd),
        prmd = const csr::PRMD,
        tf_era = const offset_of!(TrapFrame, era),
        era = const csr::ERA,
    )
}

/// 处理向量中断
/// 等效于 C 的 do_vint()
fn do_vint(_tf: &mut TrapFrame) {
    // unsigned int estat = read_csr_estat() & CSR_ESTAT_IS;
    let mut estat = estat::read().is();

    // while ((hwirq = ffs(estat)))
    // ffs (find first set) 返回第一个被设置的位的位置（1-based）
    while estat != 0 {
        // 找到第一个设置的位（从低位开始，0-based）
        let hwirq = estat.trailing_zeros() + 1;

        // estat &= ~BIT(hwirq - 1);
        // 清除已处理的位
        estat &= !(1 << (hwirq - 1));

        handle_irq(hwirq - 1);
    }
}

fn handle_irq(hwirq: u32) {
    // 处理中断的具体实现

    match hwirq {
        TI => {
            handle_timer_interrupt();
        }
        _ => {
            // 处理其他中断
        }
    }
}

static TI_HANDLER: StaticCell<fn()> = StaticCell::new(None);
pub fn register_timer_handler(handler: fn()) {
    TI_HANDLER.set(handler);
}
fn handle_timer_interrupt() {
    let h = TI_HANDLER
        .try_deref()
        .expect("Timer handler not registered");
    (h)();
}
