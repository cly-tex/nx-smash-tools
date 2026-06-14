//! Library for working with the NX system and Horizon OS
//!
//! # Note
//! Every "safe" method in this library has an implicit precondition that [`init::nx_init`]
//! has already been called to establish the runtime environment.
//!
//! Without calling that method, many of the methods in this codebase will have undefined
//! behavior and so to avoid code bloat it is assumed that the environment has been
//! properly initialized.
//!
//! # Attributions
//! Much of the code in this library is inspired by [libnx](https://github.com/switchbrew/libnx) and implementations have been referenced.
#![no_std]

extern crate alloc;

mod arm;
pub mod handle;
pub mod init;
pub mod memory;
pub mod mutex;
pub mod result;
pub mod svc;
pub mod thread;
pub mod virtmem;

pub const PAGE_SIZE: usize = 0x1000;
