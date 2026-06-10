use core::{
    ffi::{CStr, c_char},
    mem::MaybeUninit,
    time::Duration,
};

use crate::result::{NxResult, NxResultCode};

core::arch::global_asm!(include_str!("svc.s"));

unsafe extern "C" {
    fn svcSleepThread(nanos: u64);
    fn svcCloseHandle(handle: u32) -> NxResultCode;
    fn svcConnectToNamedPort(out_handle: *mut u32, port_name: *const c_char) -> NxResultCode;
    fn svcSendSyncRequest(handle: u32) -> NxResultCode;
    fn svcBreak();
}

pub fn sleep_thread(duration: Duration) {
    unsafe {
        svcSleepThread(duration.as_nanos() as u64);
    }
}

pub fn close_handle(handle: u32) -> NxResult<()> {
    let res = unsafe { svcCloseHandle(handle) };

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

pub fn break_now() {
    unsafe {
        svcBreak();
    }
}
