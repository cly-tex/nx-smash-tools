#![no_std]

core::arch::global_asm!(include_str!("crt0.s"));

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn main() {}
