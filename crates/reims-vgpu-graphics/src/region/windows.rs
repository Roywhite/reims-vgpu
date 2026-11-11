//! A sparse file mapping keeps untouched graphics pages out of Windows' commit budget.

use std::ffi::c_void;
use std::fs::OpenOptions;
use std::io;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::ptr::{NonNull, null, null_mut};
use std::sync::atomic::{AtomicU64, Ordering};

const FILE_FLAG_DELETE_ON_CLOSE: u32 = 0x0400_0000;
const FSCTL_SET_SPARSE: u32 = 0x0009_00c4;
const PAGE_READWRITE: u32 = 0x04;
const FILE_MAP_WRITE: u32 = 0x02;
static NEXT_REGION: AtomicU64 = AtomicU64::new(0);

#[link(name = "kernel32")]
unsafe extern "system" {
    fn DeviceIoControl(
        file: *mut c_void,
        control: u32,
        input: *const c_void,
        input_size: u32,
        output: *mut c_void,
        output_size: u32,
        returned: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
    fn CreateFileMappingW(
        file: *mut c_void,
        attributes: *const c_void,
        protect: u32,
        size_high: u32,
        size_low: u32,
        name: *const u16,
    ) -> *mut c_void;
    fn MapViewOfFile(
        section: *mut c_void,
        access: u32,
        offset_high: u32,
        offset_low: u32,
        length: usize,
    ) -> *mut c_void;
    fn UnmapViewOfFile(address: *const c_void) -> i32;
}

pub(super) fn map(length: usize) -> Option<*mut u8> {
    match map_sparse(length) {
        Ok(address) => Some(address),
        Err(error) => {
            log::error!("could not map the Windows graphics region: {error}");
            None
        }
    }
}

fn map_sparse(length: usize) -> io::Result<*mut u8> {
    let path = std::env::temp_dir().join(format!(
        "reims-graphics-{}-{}.ram",
        std::process::id(),
        NEXT_REGION.fetch_add(1, Ordering::Relaxed),
    ));
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .custom_flags(FILE_FLAG_DELETE_ON_CLOSE)
        .open(path)?;
    let mut returned = 0;
    // SAFETY: the live file receives a synchronous request with no input or output buffer.
    if unsafe {
        DeviceIoControl(
            file.as_raw_handle(),
            FSCTL_SET_SPARSE,
            null(),
            0,
            null_mut(),
            0,
            &mut returned,
            null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    file.set_len(length as u64)?;
    // SAFETY: zero sizes select the current file length. The OS creates an unnamed section.
    let section = NonNull::new(unsafe {
        CreateFileMappingW(file.as_raw_handle(), null(), PAGE_READWRITE, 0, 0, null())
    })
    .ok_or_else(io::Error::last_os_error)?;
    // SAFETY: CreateFileMappingW returned a new owned handle. Its view retains the section and
    // backing file independently, so both handles can close after the map.
    let section = unsafe { OwnedHandle::from_raw_handle(section.as_ptr()) };
    let view = unsafe { MapViewOfFile(section.as_raw_handle(), FILE_MAP_WRITE, 0, 0, length) };
    NonNull::new(view)
        .map(|at| at.as_ptr().cast())
        .ok_or_else(io::Error::last_os_error)
}

pub(super) unsafe fn free(address: *mut u8, _length: usize) {
    // SAFETY: the caller owns the complete view, no longer reached by the guest.
    assert_ne!(
        unsafe { UnmapViewOfFile(address.cast()) },
        0,
        "graphics region could not be released"
    );
}

#[cfg(test)]
pub(super) fn peak_resident_bytes() -> u64 {
    #[repr(C)]
    struct Counters {
        size: u32,
        faults: u32,
        peak_working_set: usize,
        remaining: [usize; 7],
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn K32GetProcessMemoryInfo(process: *mut c_void, counters: *mut Counters, size: u32)
        -> i32;
    }
    let mut counters = Counters {
        size: size_of::<Counters>() as u32,
        faults: 0,
        peak_working_set: 0,
        remaining: [0; 7],
    };
    // SAFETY: the pseudo handle names this process and counters has the SDK layout.
    assert_ne!(
        unsafe {
            K32GetProcessMemoryInfo(
                -1isize as *mut c_void,
                &mut counters,
                size_of::<Counters>() as u32,
            )
        },
        0
    );
    counters.peak_working_set as u64
}
