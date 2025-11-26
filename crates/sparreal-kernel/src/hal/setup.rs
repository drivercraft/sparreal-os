use crate::hal::al;

pub fn start_kernel() -> ! {
    crate::os::logger::init();
    info!("Setting up allocator...");

    crate::os::mem::init_heap(&al::memory::memory_map());
    al::platform::post_allocator();

    unsafe extern "C" {
        fn __sparreal_main();
    }

    unsafe { __sparreal_main() };

    al::platform::shutdown()
}
