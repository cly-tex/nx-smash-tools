use core::mem::MaybeUninit;

use crate::thread::ThreadLocalVariables;

/// Initializes the main-thread environment for use with the rest of this library
///
/// # Safety
/// - This method should **only** be called on the main thread
/// - This method should **only** be called in applications or processes where all process code
///   is written and compiled against the same version of this library
pub unsafe extern "C" fn nx_init(thread_handle: u32) {
    // We must initialize the TLS properly before doing any other initializations,
    // since we rely on the TLS being configured in such a way
    crate::thread::with_tls(|tls| {
        tls.slots_mut().fill(0);
        unsafe {
            core::ptr::write(
                tls.variables_mut(),
                ThreadLocalVariables {
                    random: MaybeUninit::uninit(),
                    thread_handle,
                },
            );
        }
    });

    // Initialize the various modules
    // SAFETY: Global random can be initializes before memory since it doesn't require any allocations
    unsafe { crate::random::init_global_rng() };

    // SAFETY: Memory needs to be initialized first, since it does not require any access to thread-local
    // variables. We are also calling it before any allocations are attempted to fulfill the safety requirements.
    unsafe { crate::memory::init() };

    // SAFETY: Now that memory has been initializes, we can initialize thread-local RNG for the main thread.
    unsafe { crate::random::init_thread_rng() };

    // SAFETY: We don't use thread-rng so this could be placed above `init_thread_rng`. We do need to initialize
    // global RNG and heap memory though
    unsafe { crate::virtmem::init() };
}
