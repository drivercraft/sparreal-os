#![no_main]

extern crate std;

use core::ffi::{c_int, c_long};
use std::{
    boxed::Box,
    string::String,
    time::Instant,
    vec,
    vec::Vec,
};

const ENOSYS: c_int = 38;

unsafe extern "C" {
    fn syscall(number: c_long, ...) -> c_long;
    fn __errno_location() -> *mut c_int;
}

#[sparreal_rt::entry]
fn main() {
    let mut values = Vec::new();
    values.push(1usize);
    values.push(2);
    values.push(3);
    assert!(values == vec![1, 2, 3], "Vec contents mismatch");

    let mut string = String::from("sparreal");
    string.push_str("-std");
    assert!(string == "sparreal-std", "String append failed");

    let boxed = Box::new(42usize);
    assert!(*boxed == 42, "Box value mismatch");

    let formatted = std::format!("{} {} {}", values.len(), string, boxed);
    assert!(
        formatted.contains("sparreal-std") && formatted.ends_with("42"),
        "format! result mismatch"
    );

    let start = Instant::now();
    let end = Instant::now();
    assert!(end >= start, "Instant should be monotonic");

    let unsupported = unsafe { syscall(0x7fff_ffff) };
    let errno = unsafe { *__errno_location() };
    assert!(unsupported == -1, "unsupported syscall should fail");
    assert!(errno == ENOSYS, "unsupported syscall should set ENOSYS");

    sparreal_rt::println!("[std-smoke] reached std::println prelude");
    std::println!("Vec: {:?}", values);
    std::println!("String: {string}");
    std::println!("Format: {formatted}");
    std::println!("All std smoke tests passed!");
    sparreal_rt::println!("[std-smoke] std::println completed");
}
