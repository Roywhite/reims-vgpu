//! The catch around a call of a host function ([`super::host_call`]), in one file per OS with the
//! same function: `catch(f)` runs `f` and answers its value, or `Err(())` for what the host threw.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(any(target_os = "linux", windows))]
mod std_unwind;

#[cfg(target_os = "macos")]
pub(super) use macos::catch;
#[cfg(any(target_os = "linux", windows))]
pub(super) use std_unwind::catch;
