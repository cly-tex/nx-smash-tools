use core::mem::offset_of;

#[repr(C, align(16))]
pub struct ThreadLocalVariables {
    pub thread_handle: u32,
}

/// Thread Local Storage
///
/// This region contains the IPC buffer, used to communicate between services/applications,
/// information for the kernel to manage the thread, and a user region used for TLS slots and
/// other SDK support.
#[repr(C)]
pub struct ThreadLocalRegion {
    ipc_buffer: [u8; 0x100],
    disable_counter: u16,
    interrupt_flag: u16,
    cache_maintenance_flag: u8,
    thread_cpu_time: u64,
    current_thread_handle: u32,
    reserved: [u8; 0x6C],
    user_region: [u8; 0x80],
}

impl ThreadLocalRegion {
    const VARIABLE_OFFSET: usize = const {
        let size_of_user_region =
            size_of::<ThreadLocalRegion>() - offset_of!(ThreadLocalRegion, user_region);

        assert!(size_of::<ThreadLocalVariables>() < size_of_user_region);

        size_of_user_region - size_of::<ThreadLocalVariables>()
    };

    const NUM_TLS_SLOTS: usize = const {
        // Check to make sure that the offset of the slot region is a multiple of usize
        assert!(offset_of!(ThreadLocalRegion, user_region) % align_of::<usize>() == 0);

        Self::VARIABLE_OFFSET / size_of::<usize>()
    };

    #[inline]
    pub fn variables(&self) -> &ThreadLocalVariables {
        unsafe {
            &*self
                .reserved
                .as_ptr()
                .add(Self::VARIABLE_OFFSET)
                .cast::<ThreadLocalVariables>()
        }
    }

    #[inline]
    pub fn variables_mut(&mut self) -> &mut ThreadLocalVariables {
        unsafe {
            &mut *self
                .reserved
                .as_mut_ptr()
                .add(Self::VARIABLE_OFFSET)
                .cast::<ThreadLocalVariables>()
        }
    }

    #[inline]
    pub fn slots(&self) -> &[usize; Self::NUM_TLS_SLOTS] {
        // SAFETY: We know the start of the slot region is aligned to usize, and we know exactly how much space we have
        unsafe { &*(&raw const self.reserved).cast::<[usize; Self::NUM_TLS_SLOTS]>() }
    }

    #[inline]
    pub fn slots_mut(&mut self) -> &mut [usize; Self::NUM_TLS_SLOTS] {
        // SAFETY: See slots impl
        unsafe { &mut *(&raw mut self.reserved).cast::<[usize; Self::NUM_TLS_SLOTS]>() }
    }

    #[inline]
    pub fn raw_ipc_buffer(&self) -> &[u8; 0x100] {
        &self.ipc_buffer
    }

    #[inline]
    pub fn raw_ipc_buffer_mut(&mut self) -> &mut [u8; 0x100] {
        &mut self.ipc_buffer
    }

    /// Write the specified value to the IPC buffer
    ///
    /// This does a const-time check to ensure that `size_of::<T>() < 0x100`
    #[inline]
    pub fn write_ipc_buffer<T: Copy + Sized>(&mut self, value: T) {
        let _: () = const { assert!(size_of::<T>() < size_of::<[u8; 0x100]>()) };

        unsafe {
            *self.ipc_buffer.as_mut_ptr().cast::<T>() = value;
        }
    }
}

const _: () = {
    assert!(size_of::<ThreadLocalRegion>() == 0x200);

    assert!(offset_of!(ThreadLocalRegion, ipc_buffer) == 0x0);
    assert!(offset_of!(ThreadLocalRegion, disable_counter) == 0x100);
    assert!(offset_of!(ThreadLocalRegion, interrupt_flag) == 0x102);
    assert!(offset_of!(ThreadLocalRegion, cache_maintenance_flag) == 0x104);
    assert!(offset_of!(ThreadLocalRegion, thread_cpu_time) == 0x108);
    assert!(offset_of!(ThreadLocalRegion, current_thread_handle) == 0x110);
    assert!(offset_of!(ThreadLocalRegion, reserved) == 0x114);
    assert!(offset_of!(ThreadLocalRegion, user_region) == 0x180);
};

/// Sets the IPC region of the TLS
///
/// This is short hand for `with_tls(|tls| tls.write_ipc_buffer(value));`
#[inline]
pub fn write_ipc_buffer<T: Copy + Sized>(value: T) {
    with_tls(move |tls| tls.write_ipc_buffer(value));
}

/// Scopes access to the TLS region to avoid Rust aliasing problems
#[inline]
pub fn with_tls<R>(f: impl FnOnce(&mut ThreadLocalRegion) -> R) -> R {
    let tls_ptr = crate::arm::tls().cast::<ThreadLocalRegion>();

    // SAFETY: The TLS pointer better be non-null or else we have serious other problems
    f(unsafe { &mut *tls_ptr })
}

#[inline]
pub fn current_thread_handle() -> u32 {
    with_tls(|tls| tls.variables().thread_handle)
}

pub enum ThreadCreationError {
    InvalidStackSize,
    InvalidStackAlignment,
}

fn create_thread(
    entry: &mut core::mem::ManuallyDrop<dyn FnOnce()>,
    stack: &mut [u8],
    priority: i32,
    core: i32,
) -> Result<(), ThreadCreationError> {
    if !stack.len().is_multiple_of(crate::PAGE_SIZE) {
        return Err(ThreadCreationError::InvalidStackSize);
    }

    // Same as `as_ptr().is_aligned_to()`
    if stack
        .as_ptr()
        .expose_provenance()
        .is_multiple_of(crate::PAGE_SIZE)
    {
        return Err(ThreadCreationError::InvalidStackAlignment);
    }

    Ok(())
}
