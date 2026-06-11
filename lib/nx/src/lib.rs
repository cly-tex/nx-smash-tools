#![no_std]

mod arm;
pub mod mutex;
pub mod result;
pub mod svc;
pub mod thread;

pub const PAGE_SIZE: usize = 0x1000;
