use crate::thread::ThreadLocalVariables;

/// Initializes the main-thread environment for use with the rest of this library
///
/// # Safety
/// - This method should **only** be called on the main thread
/// - This method should **only** be called in applications or processes where all process code
///   is written and compiled against the same version of this library
pub unsafe extern "C" fn nx_init(thread_handle: u32) {
    crate::thread::with_tls(|tls| {
        tls.slots_mut().fill(0);
        unsafe {
            core::ptr::write(tls.variables_mut(), ThreadLocalVariables { thread_handle });
        }
    });

    unsafe { crate::memory::init() };
    unsafe { crate::virtmem::init() };
}
