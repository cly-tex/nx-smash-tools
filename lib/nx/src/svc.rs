use core::{
    ffi::{CStr, c_char, c_void},
    mem::MaybeUninit,
    time::Duration,
};

use crate::{
    handle::ThreadHandle,
    result::{NxResult, NxResultCode},
};

pub mod info;

core::arch::global_asm!(include_str!("svc.s"));

#[derive(Debug, Copy, Clone)]
pub struct MemoryQuery {
    pub info: MemoryInfo,
    pub page_info: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct MemoryInfo {
    pub base_address: u64,
    pub size: u64,
    pub memory_type: u32,
    pub memory_attr: u32,
    pub memory_perm: u32,
    pub ipc_ref_count: u32,
    pub device_ref_count: u32,
}

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

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ThreadActivity {
    Runnable = 0,
    Paused = 1,
}

unsafe extern "C" {
    fn svcSetHeapSize(out_address: *mut u64, size: u64) -> NxResultCode;
    fn svcMapMemory(dst_address: u64, src_address: u64, size: u64) -> NxResultCode;
    fn svcUnmapMemory(dst_address: u64, src_address: u64, size: u64) -> NxResultCode;
    fn svcQueryMemory(
        out_info: *mut MemoryInfo,
        out_page_info: *mut u32,
        address: u64,
    ) -> NxResultCode;
    fn svcCreateThread(
        out_handle: *mut u32,
        entry: extern "C" fn(*mut c_void),
        entry_args: *mut c_void,
        stack_top: *mut c_void,
        priority: i32,
        core_id: i32,
    ) -> NxResultCode;
    fn svcStartThread(handle: u32);
    fn svcExitThread();
    fn svcSleepThread(nanos: u64);
    fn svcCloseHandle(handle: u32) -> NxResultCode;
    fn svcWaitSynchronization(
        out_signaled_handle: *mut u32,
        handles: *const u32,
        handle_count: u32,
        timeout_ns: u64,
    ) -> NxResultCode;
    fn svcCancelSynchronization(handle: u32) -> NxResultCode;
    fn svcArbitrateLock(thread_handle: u32, address: *mut u32, value: u32) -> NxResultCode;
    fn svcArbitrateUnlock(address: *mut u32) -> NxResultCode;
    fn svcConnectToNamedPort(out_handle: *mut u32, port_name: *const c_char) -> NxResultCode;
    fn svcSendSyncRequest(handle: u32) -> NxResultCode;
    fn svcBreak(reason: BreakReason, _: u64, _: u64);
    fn svcGetInfo(out: *mut u64, info_type: u32, handle: u32, info_subtype: u64) -> NxResultCode;
    fn svcSetThreadActivity(handle: u32, activity: ThreadActivity) -> NxResultCode;
}

/// Attempts to set the size of the heap region
///
/// # Safety
/// Calling this method could invalidate any allocations onto the heap that have already been made.
///
/// The caller must ensure that:
/// 1. If [`set_heap_size`] has already been called and the returned pointer is different from the previous heap pointer,
///    all existing heap allocations are invalidated immediately and no longer accessed
/// 2. If the pointer is the same, then all heap allocations past `heap_base.add(size)` are invalidated
///
/// Because both of these are very difficult to enforce in the program lifecycle, this method should
/// only be called once during the initialization of the app and before any heap is actually used.
pub unsafe fn set_heap_size(size: u64) -> NxResult<*mut c_void> {
    let mut heap_base = MaybeUninit::uninit();
    let res = unsafe { svcSetHeapSize(heap_base.as_mut_ptr(), size) };

    res.then(|| unsafe { heap_base.assume_init() } as *mut c_void)
}

pub fn map_memory(src_address: u64, dst_address: u64, size: u64) -> NxResult<()> {
    let res = unsafe { svcMapMemory(dst_address, src_address, size) };

    res.then_ok(())
}

pub unsafe fn unmap_memory(src_address: u64, dst_address: u64, size: u64) -> NxResult<()> {
    let res = unsafe { svcUnmapMemory(dst_address, src_address, size) };

    res.then_ok(())
}

pub fn query_memory(address: u64) -> NxResult<MemoryQuery> {
    let mut out_info = MaybeUninit::uninit();
    let mut out_page_info = MaybeUninit::uninit();

    let res = unsafe { svcQueryMemory(out_info.as_mut_ptr(), out_page_info.as_mut_ptr(), address) };

    res.then(|| MemoryQuery {
        info: unsafe { out_info.assume_init() },
        page_info: unsafe { out_page_info.assume_init() },
    })
}

/// # Safety
/// - Caller must ensure that `entry_args` and `stack_top` will not be released/become invalid throughout the lifetime of the thread
pub unsafe fn create_thread(
    entry: extern "C" fn(*mut c_void),
    entry_args: *mut c_void,
    stack_top: *mut c_void,
    thread_priority: i32,
    core_id: i32,
) -> NxResult<ThreadHandle> {
    let mut out_handle = MaybeUninit::uninit();

    let res = unsafe {
        svcCreateThread(
            out_handle.as_mut_ptr(),
            entry,
            entry_args,
            stack_top,
            thread_priority,
            core_id,
        )
    };

    // SAFETY: We know that the handle returned by this svc is a thread handle
    res.then(|| unsafe { ThreadHandle::new(out_handle.assume_init()) })
}

pub fn start_thread(handle: u32) {
    unsafe { svcStartThread(handle) };
}

pub fn exit_thread() -> ! {
    unsafe { svcExitThread() };
    unsafe { core::arch::asm!(".word 0xdeadbeef") };
    unsafe { core::hint::unreachable_unchecked() };
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

pub fn wait_synchronization(handles: &[u32], timeout: Option<Duration>) -> NxResult<usize> {
    let mut out_signaled_handle = MaybeUninit::uninit();
    let res = unsafe {
        svcWaitSynchronization(
            out_signaled_handle.as_mut_ptr(),
            handles.as_ptr(),
            handles.len() as u32,
            match timeout {
                Some(timeout) => timeout.as_nanos() as u64,
                None => u64::MAX,
            },
        )
    };

    res.then(|| unsafe { out_signaled_handle.assume_init() as usize })
}

pub fn cancel_synchronization(handle: u32) -> NxResult<()> {
    let res = unsafe { svcCancelSynchronization(handle) };

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

#[inline(always)]
pub fn break_now(reason: BreakReason) {
    unsafe {
        svcBreak(reason, 0, 0);
    }
}

#[inline(always)]
pub fn assert_fail() -> ! {
    break_now(BreakReason::ASSERT);
    unsafe { core::arch::asm!(".word 0xdeadbeef") };
    unsafe { core::hint::unreachable_unchecked() };
}

#[inline(always)]
pub fn set_thread_activity(handle: u32, activity: ThreadActivity) -> NxResult<()> {
    let res = unsafe { svcSetThreadActivity(handle, activity) };
    res.then_ok(())
}
