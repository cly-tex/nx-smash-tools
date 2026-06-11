#![no_std]

use core::{mem::MaybeUninit, time::Duration};

use nx::svc::BreakReason;
use services::ServiceManager;

core::arch::global_asm!(
    include_str!("crt0.s"),
    sym relocate_self,
    sym main,
);

#[unsafe(export_name = "snake_module_object")]
static mut SNAKE_MODULE_OBJECT: MaybeUninit<rtld::ModuleObject> = MaybeUninit::uninit();

#[panic_handler]
fn panic_handler(_info: &core::panic::PanicInfo) -> ! {
    nx::svc::break_now(BreakReason::PANIC);
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
    let service_manager = ServiceManager::new().unwrap();

    service_manager.register_client().unwrap();
    let _ = loop {
        match service_manager.get_service_handle(b"hid") {
            Ok(service) => break service,
            Err(0xf201) => nx::svc::sleep_thread(Duration::from_millis(50)),
            Err(e) => unsafe {
                *(e as u64 as *mut u32) = 0x69;
            },
        }
    };

    loop {
        nx::svc::sleep_thread(Duration::from_millis(50));
    }
}
