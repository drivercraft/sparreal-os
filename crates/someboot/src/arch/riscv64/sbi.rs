pub fn set_timer(stime_value: u64) -> Result<(), isize> {
    let ret = sbi_rt::set_timer(stime_value);
    if ret.error == 0 {
        Ok(())
    } else {
        Err(ret.error as isize)
    }
}

pub fn system_reset_shutdown() -> Result<(), isize> {
    let ret = sbi_rt::system_reset(sbi_rt::Shutdown, sbi_rt::NoReason);
    if ret.error == 0 {
        Ok(())
    } else {
        Err(ret.error as isize)
    }
}

pub fn detect_timebase_frequency() -> Option<usize> {
    let fdt_ptr = crate::fdt::fdt_addr()?;
    let fdt = unsafe { fdt_raw::Fdt::from_ptr(fdt_ptr).ok()? };
    let cpus = fdt.find_by_path("/cpus")?;
    let prop = cpus.find_property("timebase-frequency")?;
    prop.as_u32().map(|value| value as usize)
}
