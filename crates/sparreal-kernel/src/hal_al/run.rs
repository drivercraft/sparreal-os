use log::LevelFilter;

use ansi_rgb::{Foreground, orange};

use crate::{
    driver,
    globals::{self, PlatformInfoKind, global_val},
    io, irq,
    logger::KLogger,
    platform::{self, app_main, platform_name, shutdown},
    println, task,
};

pub fn run(plat: PlatformInfoKind) {
    platform::irq_all_disable();
    unsafe {
        if let Err(e) = globals::setup(plat) {
            println!("Global setup error: {}", e);
            shutdown();
        }
    };
    io::print::stdout_use_debug();
    println!("Kernel starting...");
    crate::mem::init();
    let _ = log::set_logger(&KLogger);
    log::set_max_level(LevelFilter::Trace);

    unsafe { globals::setup_percpu() };

    print_start_msg();

    driver::init();
    debug!("Driver initialized");
    task::init();

    irq::enable_all();

    driver::probe();

    app_main();

    shutdown()
}

macro_rules! print_pair {
    ($name:expr, $($arg:tt)*) => {
        $crate::print!("{:<30}: {}\r\n", $name, format_args!($($arg)*));
    };
}

fn print_start_msg() {
    println!("{}", LOGO.fg(orange()));

    print_pair!("Version", env!("CARGO_PKG_VERSION"));
    print_pair!("Platfrom", "{}", platform_name());
    print_pair!("Start CPU", "{}", platform::cpu_hard_id());

    match &global_val().platform_info {
        globals::PlatformInfoKind::DeviceTree(fdt) => {
            print_pair!("FDT", "{:p}", fdt.get_addr());
        }
    }

    if let Some(debug) = global_val().platform_info.debugcon()
        && let Some(c) = debug.compatibles().next()
    {
        print_pair!("Debug Serial", "{}", c);
    }
}

static LOGO: &str = r#"
     _____                                         __
    / ___/ ____   ____ _ _____ _____ ___   ____ _ / /
    \__ \ / __ \ / __ `// ___// ___// _ \ / __ `// / 
   ___/ // /_/ // /_/ // /   / /   /  __// /_/ // /  
  /____// .___/ \__,_//_/   /_/    \___/ \__,_//_/   
       /_/                                           
"#;
