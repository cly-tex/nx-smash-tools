use core::{
    ffi::{CStr, c_char},
    mem::MaybeUninit,
    time::Duration,
};

use crate::result::{NxResult, NxResultCode};

pub mod info;

core::arch::global_asm!(include_str!("svc.s"));

#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct BreakReason(u32);

impl BreakReason {
    pub const PANIC: Self = Self(0);
    pub const ASSERT: Self = Self(1);
    pub const USER: Self = Self(2);
    pub const PRE_LOAD_DLL: Self = Self(3);
    pub const POST_LOAD_DLL: Self = Self(4);
    pub const PRE_UNLOAD_DLL: Self = Self(5);
    pub const POST_UNLOAD_DLL: Self = Self(6);
    pub const CPP_EXCEPTION: Self = Self(7);

    const NOTIF_MASK: u32 = 1 << 31;

    pub const fn as_notification(self) -> Self {
        Self(self.0 | Self::NOTIF_MASK)
    }

    pub const fn is_notification(&self) -> bool {
        self.0 & Self::NOTIF_MASK != 0
    }
}

#[repr(u64)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum YieldType {
    #[default]
    NoCoreMigration = 0,
    WithCoreMigration = u64::MAX,
    AnyThread = u64::MAX - 1,
}

unsafe extern "C" {
    fn svcSleepThread(nanos: u64);
    fn svcCloseHandle(handle: u32) -> NxResultCode;
    fn svcArbitrateLock(thread_handle: u32, address: *mut u32, value: u32) -> NxResultCode;
    fn svcArbitrateUnlock(address: *mut u32) -> NxResultCode;
    fn svcConnectToNamedPort(out_handle: *mut u32, port_name: *const c_char) -> NxResultCode;
    fn svcSendSyncRequest(handle: u32) -> NxResultCode;
    fn svcBreak(reason: BreakReason, _: u64, _: u64);
    fn svcGetInfo(out: *mut u64, info_type: u32, handle: u32, info_subtype: u64) -> NxResultCode;
}

/// Yields the current thread
pub fn yield_now(ty: YieldType) {
    unsafe { svcSleepThread(ty as u64) }
}

/// Sleeps the current thread
///
/// # Notes
/// - Sleeping for any time where `nanos > u64::MAX` is not supported and could have unexpected behavior
/// - Sleeping for a zero-length duration is equivalent to [`yield_now(YieldType::NoCoreMigration)`](yield_now)
/// - Sleeping for `Duration::from_nanos(u64::MAX)` is equivalent to [`yield_now(YieldType::WithCoreMigration)`](yield_now)
/// - Sleeping for `Duration::from_nanos(u64::MAX - 1)` is equivalent to [`yield_now(YieldType::AnyThread)`](yield_now)
pub fn sleep_thread(duration: Duration) {
    unsafe {
        svcSleepThread(duration.as_nanos() as u64);
    }
}

pub fn close_handle(handle: u32) -> NxResult<()> {
    let res = unsafe { svcCloseHandle(handle) };

    res.then_ok(())
}

/// Requests the kernel to arbitrate the acquisition of a user-held lock
///
/// This is used for userland mutexes and other forms of locks where the user wants a thread to sleep
/// until it has acquired the lock.
///
/// # Arguments
/// - `owner_handle` - The handle of the thread which currently owns the lock
/// - `address` - The address of the lock
/// - `handle` - The handle to write to the lock when this thread has acquired ownership
///
/// The value of `handle` should be a proper handle, and as such it should not utilize bit 30.
/// Bit 30 is used to communicate between the kernel and the user processes that there are still waiters
/// waiting on the lock to finish.
///
/// If this method is called where the content of `address` does not have bit 30 set, control immediately passes back to the user.
/// - This prevents race-conditions in mutex/lock implementations where one thread releases the lock before another thread can request
///   kernel arbitration
#[allow(clippy::not_unsafe_ptr_arg_deref)] // We don't deref the pointer we pass it to the kernel, which does validity checks
pub fn arbitrate_lock(owner_handle: u32, address: *mut u32, value: u32) -> NxResult<()> {
    let res = unsafe { svcArbitrateLock(owner_handle, address, value) };

    res.then_ok(())
}

/// Requests the kernel to arbitrate the release of a user-held lock
///
/// This is used for userland mutexes and other forms of locks where the user needs to release the lock to the next
/// thread waiting for it
///
/// # Arguments
/// - `address` - The address of the lock
///
/// See [`arbitrate_lock`] for details about Kernel <-> userland ABI
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn arbitrate_unlock(address: *mut u32) -> NxResult<()> {
    let res = unsafe { svcArbitrateUnlock(address) };

    res.then_ok(())
}

pub fn connect_to_named_port(port_name: &CStr) -> NxResult<u32> {
    let mut handle = MaybeUninit::uninit();
    let res = unsafe { svcConnectToNamedPort(handle.as_mut_ptr(), port_name.as_ptr()) };

    res.then(|| unsafe { handle.assume_init() })
}

pub fn send_sync_request(session_handle: u32) -> NxResult<()> {
    let res = unsafe { svcSendSyncRequest(session_handle) };

    res.then_ok(())
}

pub fn break_now(reason: BreakReason) {
    unsafe {
        svcBreak(reason, 0, 0);
    }
}
