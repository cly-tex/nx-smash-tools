#![no_std]

use core::mem::MaybeUninit;

#[unsafe(export_name = "snake_module_object")]
static mut SNAKE_MODULE_OBJECT: MaybeUninit<rtld::ModuleObject> = MaybeUninit::uninit();

/// # Safety
/// This method is invoked from raw assembly during the initialization of the module,
/// it's up to that implementation to be correct in obtaining the module base
#[unsafe(no_mangle)]
unsafe extern "C" fn relocate_self(aslr_base: usize, dynamic: *const rtld::Dyn64) {
    // SAFETY: Caller upholds the safety invariants
    unsafe { rtld::relocate_raw(aslr_base, dynamic) }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn main() {}
