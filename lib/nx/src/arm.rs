use core::ffi::c_void;

pub fn tls() -> *mut c_void {
    let out: *mut c_void;

    unsafe { core::arch::asm!("mrs {}, tpidrro_el0", out(reg) out) };

    out
}
