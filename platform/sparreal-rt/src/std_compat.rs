#![allow(clippy::missing_safety_doc)]

#[cfg(not(target_arch = "aarch64"))]
compile_error!("sparreal-rt/std-compat is currently only supported on aarch64");

use core::{
    alloc::{GlobalAlloc, Layout},
    cell::UnsafeCell,
    cmp::min,
    ffi::{c_char, c_int, c_long, c_uint, c_void},
    mem::{align_of, size_of},
    ptr::null_mut,
    slice,
    time::Duration,
};

use spin::Mutex;

const EBADF: c_int = 9;
const EFAULT: c_int = 14;
const EINVAL: c_int = 22;
const ENOMEM: c_int = 12;
const ENOSYS: c_int = 38;
const ERANGE: c_int = 34;

const CLOCK_REALTIME: c_int = 0;
const CLOCK_MONOTONIC: c_int = 1;

const SYS_FUTEX: c_long = 98;
const FUTEX_WAIT: c_int = 0;
const FUTEX_WAKE: c_int = 1;
const FUTEX_WAIT_BITSET: c_int = 9;
const FUTEX_WAKE_BITSET: c_int = 10;
const FUTEX_PRIVATE_FLAG: c_int = 128;
const FUTEX_CLOCK_REALTIME: c_int = 256;

const URC_END_OF_STACK: c_int = 5;
const MAX_TLS_KEYS: usize = 64;

struct ErrnoCell(UnsafeCell<c_int>);

unsafe impl Sync for ErrnoCell {}

static ERRNO: ErrnoCell = ErrnoCell(UnsafeCell::new(0));

#[repr(C)]
pub struct Iovec {
    iov_base: *const c_void,
    iov_len: usize,
}

#[repr(C)]
pub struct Timespec {
    tv_sec: i64,
    tv_nsec: i64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct AllocHeader {
    base: *mut u8,
    alloc_size: usize,
    alloc_align: usize,
    requested_size: usize,
}

#[derive(Clone, Copy)]
struct TlsKeySlot {
    used: bool,
    value: *mut c_void,
    destructor: Option<unsafe extern "C" fn(*mut c_void)>,
}

unsafe impl Send for TlsKeySlot {}

const EMPTY_TLS_KEY: TlsKeySlot = TlsKeySlot {
    used: false,
    value: null_mut(),
    destructor: None,
};

static TLS_KEYS: Mutex<[TlsKeySlot; MAX_TLS_KEYS]> = Mutex::new([EMPTY_TLS_KEY; MAX_TLS_KEYS]);

type UnwindTraceFn = Option<unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_int>;

#[inline]
fn errno_ptr() -> *mut c_int {
    ERRNO.0.get()
}

#[inline]
fn set_errno(errno: c_int) {
    unsafe {
        *errno_ptr() = errno;
    }
}

#[inline]
fn align_up(value: usize, align: usize) -> Option<usize> {
    let mask = align.checked_sub(1)?;
    value.checked_add(mask).map(|value| value & !mask)
}

#[inline]
fn user_align(align: usize) -> usize {
    align.max(align_of::<AllocHeader>()).max(size_of::<usize>())
}

unsafe fn alloc_impl(size: usize, align: usize, zeroed: bool) -> *mut c_void {
    let requested_size = size.max(1);
    let alloc_align = user_align(align);
    let alloc_size = match requested_size
        .checked_add(size_of::<AllocHeader>())
        .and_then(|size| size.checked_add(alloc_align))
    {
        Some(size) => size,
        None => {
            set_errno(ENOMEM);
            return null_mut();
        }
    };

    let layout = match Layout::from_size_align(alloc_size, alloc_align) {
        Ok(layout) => layout,
        Err(_) => {
            set_errno(EINVAL);
            return null_mut();
        }
    };

    let base = unsafe { GlobalAlloc::alloc(crate::os::mem::kernel_memory_allocator(), layout) };
    if base.is_null() {
        set_errno(ENOMEM);
        return null_mut();
    }

    let user_addr = match align_up(base as usize + size_of::<AllocHeader>(), alloc_align) {
        Some(addr) => addr,
        None => {
            unsafe {
                GlobalAlloc::dealloc(crate::os::mem::kernel_memory_allocator(), base, layout);
            }
            set_errno(ENOMEM);
            return null_mut();
        }
    };

    let header_ptr = (user_addr - size_of::<AllocHeader>()) as *mut AllocHeader;
    unsafe {
        header_ptr.write(AllocHeader {
            base,
            alloc_size,
            alloc_align,
            requested_size,
        });
        if zeroed {
            byte_fill(user_addr as *mut u8, 0, requested_size);
        }
    }

    user_addr as *mut c_void
}

unsafe fn header_from_ptr(ptr: *mut c_void) -> *mut AllocHeader {
    (ptr as usize - size_of::<AllocHeader>()) as *mut AllocHeader
}

#[inline]
fn duration_to_timespec(duration: Duration) -> Timespec {
    Timespec {
        tv_sec: duration.as_secs() as i64,
        tv_nsec: duration.subsec_nanos() as i64,
    }
}

fn copy_message(buf: *mut c_char, buflen: usize, message: &[u8]) -> c_int {
    if buf.is_null() || buflen == 0 {
        return ERANGE;
    }

    let copy_len = min(buflen.saturating_sub(1), message.len());
    unsafe { byte_copy_nonoverlapping(message.as_ptr(), buf.cast::<u8>(), copy_len) };
    unsafe { *buf.add(copy_len) = 0 };
    0
}

#[inline]
unsafe fn byte_copy_nonoverlapping(src: *const u8, dest: *mut u8, len: usize) {
    let mut index = 0usize;
    while index < len {
        unsafe {
            *dest.add(index) = *src.add(index);
        }
        index += 1;
    }
}

#[inline]
unsafe fn byte_copy_overlap(src: *const u8, dest: *mut u8, len: usize) {
    let src_addr = src as usize;
    let dest_addr = dest as usize;
    if dest_addr <= src_addr || dest_addr >= src_addr + len {
        unsafe { byte_copy_nonoverlapping(src, dest, len) };
        return;
    }

    let mut index = len;
    while index != 0 {
        index -= 1;
        unsafe {
            *dest.add(index) = *src.add(index);
        }
    }
}

#[inline]
unsafe fn byte_fill(dest: *mut u8, value: u8, len: usize) {
    let mut index = 0usize;
    while index < len {
        unsafe {
            *dest.add(index) = value;
        }
        index += 1;
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn malloc(size: usize) -> *mut c_void {
    unsafe { alloc_impl(size, align_of::<usize>(), false) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn calloc(nmemb: usize, size: usize) -> *mut c_void {
    let total = match nmemb.checked_mul(size) {
        Some(total) => total,
        None => {
            set_errno(ENOMEM);
            return null_mut();
        }
    };
    unsafe { alloc_impl(total, align_of::<usize>(), true) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn free(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }

    let header = unsafe { *header_from_ptr(ptr) };
    let layout = match Layout::from_size_align(header.alloc_size, header.alloc_align) {
        Ok(layout) => layout,
        Err(_) => return,
    };

    unsafe {
        GlobalAlloc::dealloc(crate::os::mem::kernel_memory_allocator(), header.base, layout);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn realloc(ptr: *mut c_void, size: usize) -> *mut c_void {
    if ptr.is_null() {
        return unsafe { malloc(size) };
    }
    if size == 0 {
        unsafe { free(ptr) };
        return null_mut();
    }

    let header = unsafe { *header_from_ptr(ptr) };
    let new_ptr = unsafe { alloc_impl(size, align_of::<usize>(), false) };
    if new_ptr.is_null() {
        return null_mut();
    }

    unsafe { byte_copy_nonoverlapping(ptr.cast::<u8>(), new_ptr.cast::<u8>(), min(header.requested_size, size)) };
    unsafe { free(ptr) };

    new_ptr
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn posix_memalign(
    memptr: *mut *mut c_void,
    align: usize,
    size: usize,
) -> c_int {
    if memptr.is_null() || align < size_of::<usize>() || !align.is_power_of_two() {
        return EINVAL;
    }

    let ptr = unsafe { alloc_impl(size, align, false) };
    if ptr.is_null() {
        return ENOMEM;
    }

    unsafe {
        *memptr = ptr;
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcpy(
    dest: *mut c_void,
    src: *const c_void,
    n: usize,
) -> *mut c_void {
    unsafe { byte_copy_nonoverlapping(src.cast::<u8>(), dest.cast::<u8>(), n) };
    dest
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memmove(
    dest: *mut c_void,
    src: *const c_void,
    n: usize,
) -> *mut c_void {
    unsafe { byte_copy_overlap(src.cast::<u8>(), dest.cast::<u8>(), n) };
    dest
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memset(dest: *mut c_void, c: c_int, n: usize) -> *mut c_void {
    unsafe { byte_fill(dest.cast::<u8>(), c as u8, n) };
    dest
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcmp(lhs: *const c_void, rhs: *const c_void, n: usize) -> c_int {
    let lhs = unsafe { slice::from_raw_parts(lhs.cast::<u8>(), n) };
    let rhs = unsafe { slice::from_raw_parts(rhs.cast::<u8>(), n) };

    for (&lhs, &rhs) in lhs.iter().zip(rhs.iter()) {
        if lhs != rhs {
            return lhs as c_int - rhs as c_int;
        }
    }
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn __errno_location() -> *mut c_int {
    errno_ptr()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __xpg_strerror_r(
    errnum: c_int,
    buf: *mut c_char,
    buflen: usize,
) -> c_int {
    let message: &[u8] = match errnum {
        ENOSYS => b"Function not implemented",
        ENOMEM => b"Out of memory",
        EINVAL => b"Invalid argument",
        ERANGE => b"Result too large",
        EBADF => b"Bad file descriptor",
        _ => b"Unknown error",
    };
    copy_message(buf, buflen, message)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn write(fd: c_int, buf: *const c_void, count: usize) -> isize {
    if fd != 1 && fd != 2 {
        set_errno(EBADF);
        return -1;
    }
    if count == 0 {
        return 0;
    }
    if buf.is_null() {
        set_errno(EFAULT);
        return -1;
    }

    let bytes = unsafe { slice::from_raw_parts(buf.cast::<u8>(), count) };
    let mut written = 0usize;
    while written < bytes.len() {
        let count = somehal::console::_write_bytes(&bytes[written..]);
        if count == 0 {
            break;
        }
        written += count;
    }
    written as isize
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn writev(fd: c_int, iov: *const Iovec, iovcnt: c_int) -> isize {
    if iov.is_null() || iovcnt < 0 {
        set_errno(EINVAL);
        return -1;
    }

    let mut total = 0usize;
    for index in 0..iovcnt as usize {
        let entry = unsafe { &*iov.add(index) };
        let written = unsafe { write(fd, entry.iov_base, entry.iov_len) };
        if written < 0 {
            return -1;
        }
        total += written as usize;
    }
    total as isize
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clock_gettime(clock_id: c_int, tp: *mut Timespec) -> c_int {
    if tp.is_null() {
        set_errno(EFAULT);
        return -1;
    }

    let time = match clock_id {
        CLOCK_MONOTONIC | CLOCK_REALTIME => crate::os::time::since_boot(),
        _ => {
            set_errno(EINVAL);
            return -1;
        }
    };

    unsafe {
        tp.write(duration_to_timespec(time));
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn getcwd(buf: *mut c_char, size: usize) -> *mut c_char {
    if buf.is_null() {
        set_errno(EFAULT);
        return null_mut();
    }
    if size < 2 {
        set_errno(ERANGE);
        return null_mut();
    }

    unsafe {
        *buf = b'/' as c_char;
        *buf.add(1) = 0;
    }
    buf
}

#[unsafe(no_mangle)]
pub extern "C" fn abort() -> ! {
    crate::hal::al::platform::shutdown()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_key_create(
    key: *mut c_uint,
    destructor: Option<unsafe extern "C" fn(*mut c_void)>,
) -> c_int {
    if key.is_null() {
        return EINVAL;
    }

    let mut keys = TLS_KEYS.lock();
    for (index, slot) in keys.iter_mut().enumerate() {
        if slot.used {
            continue;
        }
        slot.used = true;
        slot.value = null_mut();
        slot.destructor = destructor;
        unsafe {
            *key = index as c_uint;
        }
        return 0;
    }

    ENOMEM
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_getspecific(key: c_uint) -> *mut c_void {
    let keys = TLS_KEYS.lock();
    keys.get(key as usize)
        .filter(|slot| slot.used)
        .map_or(null_mut(), |slot| slot.value)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_setspecific(key: c_uint, value: *const c_void) -> c_int {
    let mut keys = TLS_KEYS.lock();
    match keys.get_mut(key as usize) {
        Some(slot) if slot.used => {
            slot.value = value.cast_mut();
            0
        }
        _ => EINVAL,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_key_delete(key: c_uint) -> c_int {
    let mut keys = TLS_KEYS.lock();
    match keys.get_mut(key as usize) {
        Some(slot) if slot.used => {
            *slot = EMPTY_TLS_KEY;
            0
        }
        _ => EINVAL,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn syscall(number: c_long, mut args: ...) -> c_long {
    if number != SYS_FUTEX {
        set_errno(ENOSYS);
        return -1;
    }

    let _uaddr = unsafe { args.arg::<*const u32>() };
    let op = unsafe { args.arg::<c_int>() };
    let _val = unsafe { args.arg::<u32>() };

    let command = op & !(FUTEX_PRIVATE_FLAG | FUTEX_CLOCK_REALTIME);
    match command {
        FUTEX_WAIT | FUTEX_WAKE | FUTEX_WAIT_BITSET | FUTEX_WAKE_BITSET => 0,
        _ => {
            set_errno(ENOSYS);
            -1
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn _Unwind_Backtrace(
    _trace: UnwindTraceFn,
    _arg: *mut c_void,
) -> c_int {
    URC_END_OF_STACK
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn _Unwind_GetIP(_ctx: *const c_void) -> usize {
    0
}
