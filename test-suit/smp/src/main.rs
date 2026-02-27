#![no_main]
#![cfg_attr(not(any(windows, unix)), no_std)]
#![cfg(not(any(windows, unix)))]

use core::{
    hint::spin_loop,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use log::{error, info};
use sparreal_rt::os::{
    cpu::{CpuOnStatus, cpu_count, cpu_on},
    time::since_boot,
};

extern crate alloc;
extern crate sparreal_rt;

macro_rules! assert_test {
    ($cond:expr, $msg:expr) => {
        if !$cond {
            panic!("Test failed: {}", $msg);
        }
    };
}

static STARTED_COUNT: AtomicUsize = AtomicUsize::new(1);

#[sparreal_rt::entry]
fn __sparreal_main() {
    info!("[TEST] smp start");

    let total = cpu_count();
    info!("Detected cpu_count={}", total);
    assert_test!(total >= 4, "cpu_count should be at least 4 for qemu smp=4");

    let start_ts = since_boot();

    for cpu_id in 1..total {
        info!("Powering on CPU {}", cpu_id);
        let status = cpu_on(cpu_id);
        if status == CpuOnStatus::Ok {
            info!("cpu_on({}) ok", cpu_id);
        } else {
            error!("cpu_on({}) failed with status {:?}", cpu_id, status);
            panic!("cpu_on({}) failed with status {:?}", cpu_id, status);
        }
    }

    while STARTED_COUNT.load(Ordering::SeqCst) < total {
        if since_boot() - start_ts > Duration::from_secs(5) {
            // error!("Timeout waiting for secondary CPUs to start");
            panic!("Test failed: timeout waiting for secondary CPUs to start");
        }

        spin_loop();
    }

    info!("All tests passed!");
}

#[somehal::secondary_entry]
fn secondary() -> ! {
    info!("CPU {} started secondary entry", meta.cpu_id);
    STARTED_COUNT.fetch_add(1, Ordering::SeqCst);
    loop {
        spin_loop();
    }
}
