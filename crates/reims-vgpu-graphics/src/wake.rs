//! The graphics consumer's OS wake. Unix can poll a host doorbell descriptor;
//! Windows receives control notifications through MMIO and a native condition variable.
#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub(crate) use unix::{Wake, duplicate};
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(crate) use windows::Wake;
