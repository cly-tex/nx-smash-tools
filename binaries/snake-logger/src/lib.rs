#![no_std]

use core::{mem::MaybeUninit, time::Duration};

core::arch::global_asm!(
    include_str!("crt0.s"),
    sym relocate_self,
    sym main,
);

#[unsafe(export_name = "snake_module_object")]
static mut SNAKE_MODULE_OBJECT: MaybeUninit<rtld::ModuleObject> = MaybeUninit::uninit();

#[panic_handler]
fn panic_handler(info: &core::panic::PanicInfo) -> ! {
    nx::svc::break_now();
    loop {}
}

/// # Safety
/// This method is invoked from raw assembly during the initialization of the module,
/// it's up to that implementation to be correct in obtaining the module base
unsafe extern "C" fn relocate_self(aslr_base: usize, dynamic: *const rtld::Dyn64) {
    // SAFETY: Caller upholds the safety invariants
    unsafe { rtld::relocate_raw(aslr_base, dynamic) }
}

unsafe extern "C" fn main() {
    // Start by getting the service-manager handle
    let sm_handle = loop {
        match nx::svc::connect_to_named_port(c"sm:") {
            Ok(handle) => break handle,
            Err(0xf201) => {
                nx::svc::sleep_thread(Duration::from_millis(50));
            }
            Err(code) => {
                panic!("failed to connect to named port: {code:#x}");
            }
        }
    };

    loop {
        nx::svc::sleep_thread(Duration::from_millis(50));
    }
}
