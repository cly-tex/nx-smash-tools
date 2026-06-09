core::arch::global_asm!(include_str!("svc.s"));

unsafe extern "C" {
    fn svcBreak();
}

pub fn break_now() {
    unsafe {
        svcBreak();
    }
}
