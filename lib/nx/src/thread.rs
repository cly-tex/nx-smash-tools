use core::{
    cell::UnsafeCell,
    ffi::c_void,
    marker::PhantomData,
    mem::{ManuallyDrop, MaybeUninit, offset_of},
    time::Duration,
};

use alloc::boxed::Box;
use chacha::ChaCha;

use crate::{
    handle::ThreadHandle,
    result::{NxError, NxResult},
    virtmem::{self, VirtualReservationHandle},
};

mod spawn;

#[repr(C, align(16))]
pub struct ThreadLocalVariables {
    pub(crate) random: MaybeUninit<Box<ChaCha>>,
    pub(crate) thread_handle: u32,
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
        let _: () = const { assert!(size_of::<T>() <= size_of::<[u8; 0x100]>()) };

        unsafe {
            core::ptr::write(self.ipc_buffer.as_mut_ptr().cast(), value);
        }
    }

    /// Reads the IPC region of memory as the specified type
    ///
    /// # Safety
    /// The IPC region is raw-bytes, it's up to the caller to match the contents of the IPC
    /// buffer to the type they want to read
    #[inline]
    pub unsafe fn read_ipc_buffer<T: Copy + Sized>(&self) -> T {
        let _: () = const { assert!(size_of::<T>() <= size_of::<[u8; 0x100]>()) };

        unsafe { core::ptr::read(self.ipc_buffer.as_ptr().cast::<T>()) }
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

/// Reads the IPC region of the TLS
///
/// This is rhot hand for `with_tls(|tls| tls.read_ipc_buffer::<T>())`
///
/// # Safety
/// See [`ThreadLocalRegion::read_ipc_buffer`] for safety information
#[inline]
pub unsafe fn read_ipc_buffer<T: Copy + Sized>() -> T {
    // SAFETY: Caller upholds safety requirements
    with_tls(|tls| unsafe { tls.read_ipc_buffer::<T>() })
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
    StackAllocationFailed(NxError),
    CreateThreadError(NxError),
}

struct ThreadContext<R: Send, F: FnOnce() -> R + Send> {
    entry_args: ThreadEntryArgs<R, F>,
    user_callback: ManuallyDrop<F>,
    return_storage: MaybeUninit<R>,
}

struct ThreadEntryArgs<R: Send, F: FnOnce() -> R + Send> {
    user_callback: *mut ManuallyDrop<F>,
    return_storage: *mut MaybeUninit<R>,
}

extern "C" fn thread_entrypoint<R: Send, F: FnOnce() -> R + Send>(thread_context: *mut c_void) {
    let context = unsafe { &mut *thread_context.cast::<ThreadEntryArgs<R, F>>() };

    let callback = unsafe { ManuallyDrop::take(&mut *context.user_callback) };

    unsafe {
        (*context.return_storage).write(callback());
    }
}

pub enum WaitTimeoutError<T> {
    TimedOut(T),
    Other(NxError),
}

pub struct JoinHandle<R: Send + 'static>(JoinHandleInner<R>);

impl<R: Send + 'static> JoinHandle<R> {
    pub fn wait_timeout(self, timeout: Duration) -> Result<R, WaitTimeoutError<Self>> {
        self.0.wait_timeout(timeout).map_err(|e| match e {
            WaitTimeoutError::TimedOut(handle) => WaitTimeoutError::TimedOut(Self(handle)),
            WaitTimeoutError::Other(e) => WaitTimeoutError::Other(e),
        })
    }

    pub fn wait(self) -> NxResult<R> {
        self.0.wait()
    }
}

pub struct LocalJoinHandle<'a, R: Send + 'a> {
    inner: JoinHandleInner<R>,
    marker: PhantomData<&'a ()>,
}

impl<'a, R: Send + 'a> LocalJoinHandle<'a, R> {
    pub fn wait_timeout(self, timeout: Duration) -> Result<R, WaitTimeoutError<Self>> {
        self.inner.wait_timeout(timeout).map_err(|e| match e {
            WaitTimeoutError::TimedOut(handle) => WaitTimeoutError::TimedOut(Self {
                inner: handle,
                marker: PhantomData,
            }),
            WaitTimeoutError::Other(e) => WaitTimeoutError::Other(e),
        })
    }

    pub fn wait(self) -> NxResult<R> {
        self.inner.wait()
    }
}

struct JoinHandleInner<R: Send> {
    thread_handle: ThreadHandle,
    virtmem_handle: VirtualReservationHandle,
    return_storage: *mut MaybeUninit<R>,
}

impl<R: Send> JoinHandleInner<R> {
    pub fn wait_timeout(self, timeout: Duration) -> Result<R, WaitTimeoutError<Self>> {
        match crate::svc::wait_synchronization(&[self.thread_handle.into_inner()], Some(timeout)) {
            Ok(_) => Ok(unsafe { (*self.return_storage).assume_init_read() }),
            Err(crate::result::svc::TIMED_OUT) => Err(WaitTimeoutError::TimedOut(self)),
            Err(other) => Err(WaitTimeoutError::Other(other)),
        }
    }

    pub fn wait(self) -> NxResult<R> {
        crate::svc::wait_synchronization(&[self.thread_handle.into_inner()], None)
            .map(move |_| unsafe { (*self.return_storage).assume_init_read() })
    }
}

fn create_thread<'a, R: Send + 'a, F: FnOnce() -> R + Send + 'a>(
    entry: F,
    stack: *mut u8,
    stack_size: usize,
    priority: i32,
    core: i32,
) -> Result<JoinHandleInner<R>, ThreadCreationError> {
    if !stack_size.is_multiple_of(crate::PAGE_SIZE) {
        return Err(ThreadCreationError::InvalidStackSize);
    }

    let modified_stack_size = size_of::<ThreadContext<R, F>>() + stack_size;
    let modified_stack_size =
        (modified_stack_size + crate::PAGE_SIZE - 1) & !(crate::PAGE_SIZE - 1);

    // Same as `as_ptr().is_aligned_to()`
    if stack.expose_provenance().is_multiple_of(crate::PAGE_SIZE) {
        return Err(ThreadCreationError::InvalidStackAlignment);
    }

    let virtmem = unsafe {
        virtmem::map(stack, modified_stack_size, virtmem::AllocationType::Stack)
            .map_err(ThreadCreationError::StackAllocationFailed)?
    };

    let stack_top = unsafe {
        virtmem
            .as_ptr()
            .add(modified_stack_size - size_of::<ThreadContext<R, F>>())
    };

    let context = stack_top.cast::<ThreadContext<R, F>>();
    let user_callback_ptr = unsafe {
        stack_top
            .add(offset_of!(ThreadContext::<R, F>, user_callback))
            .cast::<ManuallyDrop<F>>()
    };
    let return_storage_ptr = unsafe {
        stack_top
            .add(offset_of!(ThreadContext::<R, F>, return_storage))
            .cast::<MaybeUninit<R>>()
    };

    unsafe {
        core::ptr::write(
            context,
            ThreadContext {
                entry_args: ThreadEntryArgs {
                    user_callback: user_callback_ptr,
                    return_storage: return_storage_ptr,
                },
                user_callback: ManuallyDrop::new(entry),
                return_storage: MaybeUninit::uninit(),
            },
        );
    }

    // SAFETY: If this SVC is successful then we will preserve the virmem allocation throughout the lifetime of the thread handle
    let handle = match unsafe {
        crate::svc::create_thread(
            thread_entrypoint::<R, F>,
            context.cast(),
            stack_top.cast(),
            priority,
            core,
        )
    } {
        Ok(thread_handle) => thread_handle,
        Err(e) => {
            virtmem::unmap(virtmem);

            return Err(ThreadCreationError::CreateThreadError(e));
        }
    };

    Ok(())
}

pub struct LocalThreadGroup {
    terminate_list: UnsafeCell<[u32; 0x40]>,
    terminate_cursor: UnsafeCell<usize>,
}

impl LocalThreadGroup {
    pub fn builder<'a, F: FnOnce() -> R + 'a, R: Send + 'a>(
        &'a self,
    ) -> LocalGroupThreadBuilder<'a, F, R> {
        todo!()
    }

    pub fn spawn<'a, R: Send + 'a>(&'a self, f: impl FnOnce() -> R + 'a) -> LocalJoinHandle<'a, R> {
        todo!()
    }
}

pub struct LocalGroupPendingThread<'a, R: Send + 'a> {
    marker: PhantomData<&'a R>,
}

pub struct LocalGroupThreadBuilder<'a, F: FnOnce() -> R + 'a, R: Send + 'a> {
    marker: PhantomData<&'a F>,
}

pub struct PendingThread<R: Send + 'static> {
    marker: PhantomData<R>,
}

pub struct ThreadBuilder<F: FnOnce() -> R + 'static, R: Send + 'static> {
    stack: *mut u8,
    stack_size: usize,
    priority: i32,
    core_id: i32,
    marker: PhantomData<F>,
}

impl<F: FnOnce() -> R + 'static, R: Send + 'static> ThreadBuilder<F, R> {
    pub fn with_stack(mut self, ptr: *mut u8, size: usize) -> Self {
        self.stack = ptr;
        self.stack_size = size;
        self
    }

    pub fn with_priority(mut self, prio: i32) -> Self {
        self.priority = prio;
        self
    }

    pub fn with_core(mut self, core_id: i32) -> Self {
        self.core_id = core_id;
        self
    }

    pub fn build(self, entry: F) -> Result<PendingThread<R>, ThreadCreationError> {
        todo!()
    }
}
