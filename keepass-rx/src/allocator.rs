use std::ffi::{CStr, c_int, c_void};
use std::fs::{create_dir_all, write};
use std::io;
use std::mem::transmute;
use std::path::PathBuf;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicPtr, AtomicU8, Ordering};

use hardened_malloc_sys as _;
use libc::{__errno_location, EINVAL, ENOMEM, RTLD_NEXT, dlsym, getenv};
use rustix::fs::{Mode, OFlags, open};
use rustix::io::read;

use crate::APP_ID;

const UNDECIDED: u8 = 0;
const SYSTEM: u8 = 1;
const HARDENED: u8 = 2;

const FLAG_FILE: &str = "hardened-allocator";
const DEVICE_PROPS: &CStr = c"/vendor/build.prop";
const DEVICE_KEY: &[u8] = b"ro.product.vendor.device=";
const HARDENED_DEVICES: [&[u8]; 1] = [b"JP2601"];
const PATH_MAX: usize = 1024;
const CHUNK: usize = 4096;

static CHOICE: AtomicU8 = AtomicU8::new(UNDECIDED);
static SYSTEM_USABLE_SIZE: AtomicPtr<c_void> = AtomicPtr::new(null_mut());

unsafe extern "C" {
    fn h_malloc(size: usize) -> *mut c_void;
    fn h_calloc(count: usize, size: usize) -> *mut c_void;
    fn h_realloc(ptr: *mut c_void, size: usize) -> *mut c_void;
    fn h_reallocarray(ptr: *mut c_void, count: usize, size: usize) -> *mut c_void;
    fn h_free(ptr: *mut c_void);
    fn h_posix_memalign(out: *mut *mut c_void, alignment: usize, size: usize) -> c_int;
    fn h_aligned_alloc(alignment: usize, size: usize) -> *mut c_void;
    fn h_memalign(alignment: usize, size: usize) -> *mut c_void;
    fn h_valloc(size: usize) -> *mut c_void;
    fn h_pvalloc(size: usize) -> *mut c_void;
    fn h_malloc_usable_size(ptr: *mut c_void) -> usize;

    fn __libc_malloc(size: usize) -> *mut c_void;
    fn __libc_calloc(count: usize, size: usize) -> *mut c_void;
    fn __libc_realloc(ptr: *mut c_void, size: usize) -> *mut c_void;
    fn __libc_free(ptr: *mut c_void);
    fn __libc_memalign(alignment: usize, size: usize) -> *mut c_void;
    fn __libc_valloc(size: usize) -> *mut c_void;
    fn __libc_pvalloc(size: usize) -> *mut c_void;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Allocator {
    System,
    Hardened,
}

pub fn active() -> Allocator {
    if hardened() {
        Allocator::Hardened
    } else {
        Allocator::System
    }
}

pub fn configured() -> bool {
    flag().unwrap_or_else(device_default)
}

pub fn set_configured(enabled: bool) -> io::Result<()> {
    let dir = data_dir().ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
    create_dir_all(&dir)?;
    write(dir.join(FLAG_FILE), if enabled { "1" } else { "0" })
}

fn data_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|dir| dir.join(APP_ID))
}

fn hardened() -> bool {
    let choice = match CHOICE.load(Ordering::Acquire) {
        UNDECIDED => decide(),
        choice => choice,
    };
    choice == HARDENED
}

fn decide() -> u8 {
    let wanted = if configured() { HARDENED } else { SYSTEM };
    match CHOICE.compare_exchange(UNDECIDED, wanted, Ordering::AcqRel, Ordering::Acquire) {
        Ok(_) => wanted,
        Err(existing) => existing,
    }
}

fn env(name: &CStr) -> Option<&'static [u8]> {
    let value = unsafe { getenv(name.as_ptr()) };
    if value.is_null() {
        return None;
    }
    Some(unsafe { CStr::from_ptr(value) }.to_bytes()).filter(|value| !value.is_empty())
}

fn flag_path(buf: &mut [u8; PATH_MAX]) -> Option<&CStr> {
    let parts: [&[u8]; 6] = match env(c"XDG_DATA_HOME") {
        Some(data) => [
            data,
            b"/",
            APP_ID.as_bytes(),
            b"/",
            FLAG_FILE.as_bytes(),
            b"\0",
        ],
        None => [
            env(c"HOME")?,
            b"/.local/share/",
            APP_ID.as_bytes(),
            b"/",
            FLAG_FILE.as_bytes(),
            b"\0",
        ],
    };
    let mut len = 0;
    for part in parts {
        let end = len + part.len();
        buf.get_mut(len..end)?.copy_from_slice(part);
        len = end;
    }
    CStr::from_bytes_with_nul(&buf[..len]).ok()
}

fn flag() -> Option<bool> {
    let mut path = [0u8; PATH_MAX];
    let fd = open(
        flag_path(&mut path)?,
        OFlags::RDONLY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .ok()?;
    let mut byte = [0u8; 1];
    match read(&fd, &mut byte) {
        Ok(1) if byte[0] == b'1' => Some(true),
        Ok(1) if byte[0] == b'0' => Some(false),
        _ => None,
    }
}

fn device_default() -> bool {
    let Ok(fd) = open(
        DEVICE_PROPS,
        OFlags::RDONLY | OFlags::CLOEXEC,
        Mode::empty(),
    ) else {
        return false;
    };
    let mut buf = [0u8; CHUNK * 2];
    let mut kept = 0;
    loop {
        let Ok(count) = read(&fd, &mut buf[kept..kept + CHUNK]) else {
            return false;
        };
        let filled = kept + count;
        let end = if count == 0 {
            filled
        } else {
            buf[..filled]
                .iter()
                .rposition(|byte| *byte == b'\n')
                .map_or(0, |newline| newline + 1)
        };
        if names_hardened_device(&buf[..end]) {
            return true;
        }
        if count == 0 {
            return false;
        }
        kept = filled - end;
        if kept > CHUNK {
            kept = 0;
        } else {
            buf.copy_within(end..filled, 0);
        }
    }
}

fn names_hardened_device(props: &[u8]) -> bool {
    HARDENED_DEVICES.iter().any(|device| {
        props.split(|byte| *byte == b'\n').any(|line| {
            line.strip_prefix(DEVICE_KEY)
                .is_some_and(|value| value.trim_ascii() == *device)
        })
    })
}

fn system_usable_size(ptr: *mut c_void) -> usize {
    let mut found = SYSTEM_USABLE_SIZE.load(Ordering::Acquire);
    if found.is_null() {
        found = unsafe { dlsym(RTLD_NEXT, c"malloc_usable_size".as_ptr()) };
        SYSTEM_USABLE_SIZE.store(found, Ordering::Release);
    }
    if found.is_null() {
        return 0;
    }
    let usable_size: unsafe extern "C" fn(*mut c_void) -> usize = unsafe { transmute(found) };
    unsafe { usable_size(ptr) }
}

fn set_errno(value: c_int) {
    unsafe { *__errno_location() = value };
}

#[unsafe(no_mangle)]
unsafe extern "C" fn malloc(size: usize) -> *mut c_void {
    if hardened() {
        unsafe { h_malloc(size) }
    } else {
        unsafe { __libc_malloc(size) }
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn calloc(count: usize, size: usize) -> *mut c_void {
    if hardened() {
        unsafe { h_calloc(count, size) }
    } else {
        unsafe { __libc_calloc(count, size) }
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn realloc(ptr: *mut c_void, size: usize) -> *mut c_void {
    if hardened() {
        unsafe { h_realloc(ptr, size) }
    } else {
        unsafe { __libc_realloc(ptr, size) }
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn reallocarray(ptr: *mut c_void, count: usize, size: usize) -> *mut c_void {
    if hardened() {
        return unsafe { h_reallocarray(ptr, count, size) };
    }
    match count.checked_mul(size) {
        Some(total) => unsafe { __libc_realloc(ptr, total) },
        None => {
            set_errno(ENOMEM);
            null_mut()
        }
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn free(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    if hardened() {
        unsafe { h_free(ptr) }
    } else {
        unsafe { __libc_free(ptr) }
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn posix_memalign(
    out: *mut *mut c_void,
    alignment: usize,
    size: usize,
) -> c_int {
    if hardened() {
        return unsafe { h_posix_memalign(out, alignment, size) };
    }
    if !alignment.is_power_of_two() || !alignment.is_multiple_of(size_of::<*mut c_void>()) {
        return EINVAL;
    }
    let ptr = unsafe { __libc_memalign(alignment, size) };
    if ptr.is_null() {
        return ENOMEM;
    }
    unsafe { *out = ptr };
    0
}

#[unsafe(no_mangle)]
unsafe extern "C" fn aligned_alloc(alignment: usize, size: usize) -> *mut c_void {
    if hardened() {
        return unsafe { h_aligned_alloc(alignment, size) };
    }
    if !alignment.is_power_of_two() {
        set_errno(EINVAL);
        return null_mut();
    }
    unsafe { __libc_memalign(alignment, size) }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn memalign(alignment: usize, size: usize) -> *mut c_void {
    if hardened() {
        unsafe { h_memalign(alignment, size) }
    } else {
        unsafe { __libc_memalign(alignment, size) }
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn valloc(size: usize) -> *mut c_void {
    if hardened() {
        unsafe { h_valloc(size) }
    } else {
        unsafe { __libc_valloc(size) }
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn pvalloc(size: usize) -> *mut c_void {
    if hardened() {
        unsafe { h_pvalloc(size) }
    } else {
        unsafe { __libc_pvalloc(size) }
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn malloc_usable_size(ptr: *mut c_void) -> usize {
    if ptr.is_null() {
        return 0;
    }
    if hardened() {
        unsafe { h_malloc_usable_size(ptr) }
    } else {
        system_usable_size(ptr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_device_line() {
        let props = b"ro.a=b\nro.product.vendor.device=JP2601\nro.c=d\n";
        assert!(names_hardened_device(props));
        assert!(!names_hardened_device(
            b"ro.product.vendor.device=JP2601X\n"
        ));
        assert!(!names_hardened_device(b"ro.product.vendor.model=JP2601\n"));
    }
}
