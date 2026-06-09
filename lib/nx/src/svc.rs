use core::{
    ffi::{CStr, c_char},
    mem::MaybeUninit,
    time::Duration,
};

core::arch::global_asm!(include_str!("svc.s"));

unsafe extern "C" {
    fn svcSleepThread(nanos: u64);
    fn svcCloseHandle(handle: u32) -> u32;
    fn svcConnectToNamedPort(out_handle: *mut u32, port_name: *const c_char) -> u32;
    fn svcSendSyncRequest(handle: u32) -> u32;
    fn svcBreak();
}

pub fn sleep_thread(duration: Duration) {
    unsafe {
        svcSleepThread(duration.as_nanos() as u64);
    }
}

pub fn close_handle(handle: u32) -> Result<(), u32> {
    let res = unsafe { svcCloseHandle(handle) };

    if core::hint::likely(res == 0) {
        Ok(())
    } else {
        Err(res)
    }
}

pub fn connect_to_named_port(port_name: &CStr) -> Result<u32, u32> {
    let mut handle = MaybeUninit::uninit();
    let res = unsafe { svcConnectToNamedPort(handle.as_mut_ptr(), port_name.as_ptr()) };

    if core::hint::likely(res == 0) {
        Ok(unsafe { handle.assume_init() })
    } else {
        Err(res)
    }
}

pub fn send_sync_request(session_handle: u32) -> Result<(), u32> {
    let res = unsafe { svcSendSyncRequest(session_handle) };

    if core::hint::likely(res == 0) {
        Ok(())
    } else {
        Err(res)
    }
}

pub fn break_now() {
    unsafe {
        svcBreak();
    }
}
