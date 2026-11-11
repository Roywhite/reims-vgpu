//! Unix anonymous storage for a graphics region.

pub(super) fn map(length: usize) -> Option<*mut u8> {
    // SAFETY: a fresh private, writable anonymous mapping at the kernel's address.
    let at = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            length,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_PRIVATE | libc::MAP_ANON,
            -1,
            0,
        )
    };
    (at != libc::MAP_FAILED).then_some(at.cast())
}

pub(super) unsafe fn free(address: *mut u8, length: usize) {
    // SAFETY: the caller owns this complete mapping and has retired its users.
    assert_eq!(unsafe { libc::munmap(address.cast(), length) }, 0);
}
