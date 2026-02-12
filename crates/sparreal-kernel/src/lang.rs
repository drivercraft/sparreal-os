use core::panic::PanicInfo;

use crate::{hal::al::platform::shutdown, println};

#[allow(dead_code)]
#[cfg_attr(not(any(windows, unix)), panic_handler)]
fn panic(info: &PanicInfo) -> ! {
    println!("Panicked: {info}");

    shutdown()
}
