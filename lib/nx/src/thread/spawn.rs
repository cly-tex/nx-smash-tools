use core::{
    ffi::c_void,
    marker::PhantomData,
    mem::{ManuallyDrop, MaybeUninit, offset_of},
    ptr::NonNull,
    sync::atomic::{AtomicU32, Ordering},
    time::Duration,
};

use crate::{
    handle::ThreadHandle,
    result::NxError,
    svc::ThreadActivity,
    virtmem::{self, VirtualReservationHandle},
};

const THREAD_STATE_RUNNING: u32 = 0u32;
const THREAD_STATE_FINISHED: u32 = 1u32;
const THREAD_STATE_DETACHED: u32 = 2u32;

#[derive(Debug, Copy, Clone)]
pub enum ThreadWaitErrorReason {
    /// A different thread has requested the waiting thread to cancel it's wait request
    Cancelled,

    /// The process is being terminated so the wait must be stopped
    Termination,

    /// Waiting on the thread timed out
    TimedOut,
}

pub struct ThreadWaitError<R: Send> {
    pub reason: ThreadWaitErrorReason,
    pub thread: SpawnedThread<R>,
}

pub type ThreadWaitResult<R> = Result<R, ThreadWaitError<R>>;

struct OwnedStackAllocation {
    memory: NonNull<u8>,
    size: usize,
}

impl Drop for OwnedStackAllocation {
    fn drop(&mut self) {
        let layout =
            unsafe { alloc::alloc::Layout::from_size_align_unchecked(self.size, crate::PAGE_SIZE) };
        unsafe { alloc::alloc::dealloc(self.memory.as_ptr(), layout) };
    }
}

struct ThreadInner<R: Send> {
    handle: ThreadHandle,
    virtmem_reservation: ManuallyDrop<VirtualReservationHandle>,
    owned_memory: Option<OwnedStackAllocation>,
    return_storage: *mut MaybeUninit<R>,
    thread_state: *const AtomicU32,
}

pub struct SpawnedThread<R: Send>(ThreadInner<R>);

unsafe impl<R: Send> Send for SpawnedThread<R> {}
unsafe impl<R: Send> Sync for SpawnedThread<R> {}

impl<R: Send> SpawnedThread<R> {
    fn release_resources(&mut self) {
        // FIXME: virtmem::unmap should not return a result since the reservation data can only be constructed
        // from within the module
        if virtmem::unmap(unsafe { ManuallyDrop::take(&mut self.0.virtmem_reservation) }).is_err() {
            crate::svc::assert_fail();
        }

        if crate::svc::close_handle(self.0.handle.into_inner()).is_err() {
            crate::svc::assert_fail();
        }
    }

    pub fn unpause(&self) {
        if crate::svc::set_thread_activity(self.0.handle.into_inner(), ThreadActivity::Runnable)
            .is_err()
        {
            crate::svc::assert_fail();
        }
    }

    pub fn pause(&self) {
        if crate::svc::set_thread_activity(self.0.handle.into_inner(), ThreadActivity::Paused)
            .is_err()
        {
            crate::svc::assert_fail();
        }
    }

    pub fn unstuck(&self) {
        if crate::svc::cancel_synchronization(self.0.handle.into_inner()).is_err() {
            crate::svc::assert_fail();
        }
    }

    pub fn wait(mut self, timeout: Option<Duration>) -> ThreadWaitResult<R> {
        match crate::svc::wait_synchronization(&[self.0.handle.into_inner()], timeout) {
            Ok(_) => {
                let retval = unsafe { (*self.0.return_storage).assume_init_read() };
                self.release_resources();
                core::mem::forget(self);
                Ok(retval)
            }
            Err(e) => {
                let reason = match e {
                    crate::result::svc::CANCELLED => ThreadWaitErrorReason::Cancelled,
                    crate::result::svc::TERMINATION_REQUESTED => ThreadWaitErrorReason::Termination,
                    crate::result::svc::TIMED_OUT => ThreadWaitErrorReason::TimedOut,
                    _ => crate::svc::assert_fail(),
                };

                Err(ThreadWaitError {
                    reason,
                    thread: self,
                })
            }
        }
    }
}

impl<R: Send> Drop for SpawnedThread<R> {
    fn drop(&mut self) {
        /* Notes on thread detaching:
         * 1. There needs to be a memory-write barrier between writing the return value storage and updating the thread state from the thread side
         * 2. There needs to be a memory-reaad barrier between reading the thread state and reading the return value storage from the detacher side
         *
         * Since we are the detacher, we use Ordering::Relaxed on the success (thread is currently running) since we aren't
         * reading from a shared memory location, and Ordering::Acquire on the fail (thread has finished) since we will be reading
         * from the shared storage
         */
        match unsafe {
            (*self.0.thread_state).compare_exchange(
                THREAD_STATE_RUNNING,
                THREAD_STATE_DETACHED,
                Ordering::Relaxed,
                Ordering::Acquire,
            )
        } {
            Ok(_) => {}
            Err(THREAD_STATE_FINISHED) => {
                unsafe { (*self.0.return_storage).assume_init_drop() };
                self.release_resources();
            }
            Err(_) => crate::svc::assert_fail(),
        }
    }
}

/// Thread which has been created but not yet started
pub struct PendingThread<R: Send>(ManuallyDrop<ThreadInner<R>>);

impl<R: Send> PendingThread<R> {
    pub fn spawn(mut self) -> SpawnedThread<R> {
        crate::svc::start_thread(self.0.handle.into_inner());

        let out = SpawnedThread(unsafe { ManuallyDrop::take(&mut self.0) });
        core::mem::forget(self);
        out
    }
}

unsafe impl<R: Send> Send for PendingThread<R> {}
unsafe impl<R: Send> Sync for PendingThread<R> {}

impl<R: Send> Drop for PendingThread<R> {
    fn drop(&mut self) {
        // FIXME: virtmem::unmap should not return a result since the reservation data can only be constructed
        // from within the module
        if virtmem::unmap(unsafe { ManuallyDrop::take(&mut self.0.virtmem_reservation) }).is_err() {
            crate::svc::assert_fail();
        }

        if crate::svc::close_handle(self.0.handle.into_inner()).is_err() {
            crate::svc::assert_fail();
        }
    }
}

#[repr(align(16))]
struct ThreadContext<R: Send, F: FnOnce() -> R + Send> {
    entry_args: ThreadEntryArgs<R, F>,
    user_callback: ManuallyDrop<F>,
    return_storage: MaybeUninit<R>,
    thread_state: AtomicU32,
}

struct ThreadEntryArgs<R: Send, F: FnOnce() -> R + Send> {
    user_callback: *mut ManuallyDrop<F>,
    return_storage: *mut MaybeUninit<R>,
    thread_state: *const AtomicU32,
}

extern "C" fn thread_entrypoint<R: Send, F: FnOnce() -> R + Send>(thread_context: *mut c_void) {
    let context = unsafe { &mut *thread_context.cast::<ThreadEntryArgs<R, F>>() };

    let callback = unsafe { ManuallyDrop::take(&mut *context.user_callback) };

    unsafe {
        (*context.return_storage).write(callback());
    }

    match unsafe {
        (*context.thread_state).compare_exchange(
            THREAD_STATE_RUNNING,
            THREAD_STATE_FINISHED,
            Ordering::Release,
            Ordering::Relaxed,
        )
    } {
        Ok(_) => {}
        Err(THREAD_STATE_DETACHED) => {
            unsafe { (*context.return_storage).assume_init_drop() };
        }
        Err(_) => crate::svc::assert_fail(),
    }

    crate::svc::exit_thread();
}

pub enum ThreadCreationError {
    InvalidStackSize,
    InvalidStackAlignment,
    StackAllocationFailed(NxError),
    CreateThreadError(NxError),
}

pub struct ThreadBuilder<F: FnOnce() -> R + Send, R: Send> {
    stack: Option<NonNull<u8>>,
    stack_size: usize,
    priority: i32,
    core_id: i32,
    marker: PhantomData<(F, R)>,
}

impl<F: FnOnce() -> R + Send, R: Send> ThreadBuilder<F, R> {
    const DEFAULT_STACK_SIZE: usize = 0x4000;

    pub const fn new() -> Self {
        Self {
            stack: None,
            stack_size: Self::DEFAULT_STACK_SIZE,
            priority: 16,
            core_id: -2,
            marker: PhantomData,
        }
    }

    /// # Safety
    /// If `ptr` is not `None`, then the caller must ensure:
    /// - the memory pointed to by `ptr` is valid for reads/writes through `ptr.add(size)`
    /// - the memory pointed to by `ptr` will be valid for the duration of the thread's life
    pub const unsafe fn with_stack(self, ptr: Option<NonNull<u8>>, size: usize) -> Self {
        Self {
            stack: ptr,
            stack_size: size,
            ..self
        }
    }

    pub const fn with_stack_size(self, size: usize) -> Self {
        unsafe { self.with_stack(None, size) }
    }

    pub const fn with_priority(self, prio: i32) -> Self {
        Self {
            priority: prio,
            ..self
        }
    }

    pub const fn with_core_id(self, core_id: i32) -> Self {
        Self { core_id, ..self }
    }

    pub fn spawn(self, entry: F) -> Result<SpawnedThread<R>, ThreadCreationError> {
        self.build(entry).map(|pending| pending.spawn())
    }

    pub fn build(self, entry: F) -> Result<PendingThread<R>, ThreadCreationError> {
        let _: () = const { assert!(align_of::<ThreadContext<R, F>>() == 0x10) };

        if !self.stack_size.is_multiple_of(crate::PAGE_SIZE)
            || size_of::<ThreadContext<R, F>>() >= self.stack_size
        {
            return Err(ThreadCreationError::InvalidStackSize);
        }

        let owned_stack = self.stack.is_none();
        let stack_ptr = match self.stack {
            Some(ptr) => {
                if ptr
                    .expose_provenance()
                    .get()
                    .is_multiple_of(crate::PAGE_SIZE)
                {
                    ptr.as_ptr()
                } else {
                    return Err(ThreadCreationError::InvalidStackAlignment);
                }
            }
            None => {
                let layout = unsafe {
                    alloc::alloc::Layout::from_size_align_unchecked(
                        self.stack_size,
                        crate::PAGE_SIZE,
                    )
                };
                let ptr = unsafe { alloc::alloc::alloc(layout) };
                if ptr.is_null() {
                    crate::svc::assert_fail();
                }
                ptr
            }
        };

        let owned_stack = owned_stack.then(|| OwnedStackAllocation {
            memory: unsafe { NonNull::new_unchecked(stack_ptr) },
            size: self.stack_size,
        });

        // Same as `as_ptr().is_aligned_to()`
        if stack_ptr
            .expose_provenance()
            .is_multiple_of(crate::PAGE_SIZE)
        {
            return Err(ThreadCreationError::InvalidStackAlignment);
        }

        let virtmem = unsafe {
            virtmem::map(stack_ptr, self.stack_size, virtmem::AllocationType::Stack)
                .map_err(ThreadCreationError::StackAllocationFailed)?
        };

        let stack_top = unsafe {
            virtmem
                .as_ptr()
                .add(self.stack_size - size_of::<ThreadContext<R, F>>())
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
        let thread_state_ptr = unsafe {
            stack_top
                .add(offset_of!(ThreadContext::<R, F>, thread_state))
                .cast::<AtomicU32>()
        };

        unsafe {
            core::ptr::write(
                context,
                ThreadContext {
                    entry_args: ThreadEntryArgs {
                        user_callback: user_callback_ptr,
                        return_storage: return_storage_ptr,
                        thread_state: thread_state_ptr,
                    },
                    user_callback: ManuallyDrop::new(entry),
                    return_storage: MaybeUninit::uninit(),
                    thread_state: AtomicU32::new(THREAD_STATE_RUNNING),
                },
            );
        }

        // SAFETY: If this SVC is successful then we will preserve the virmem allocation throughout the lifetime of the thread handle
        let handle = match unsafe {
            crate::svc::create_thread(
                thread_entrypoint::<R, F>,
                context.cast(),
                stack_top.cast(),
                self.priority,
                self.core_id,
            )
        } {
            Ok(thread_handle) => thread_handle,
            Err(e) => {
                virtmem::unmap(virtmem);

                return Err(ThreadCreationError::CreateThreadError(e));
            }
        };

        Ok(PendingThread(ManuallyDrop::new(ThreadInner {
            handle,
            virtmem_reservation: ManuallyDrop::new(virtmem),
            owned_memory: owned_stack,
            return_storage: return_storage_ptr,
            thread_state: thread_state_ptr,
        })))
    }
}

impl<F: FnOnce() -> R + Send, R: Send> Default for ThreadBuilder<F, R> {
    fn default() -> Self {
        Self::new()
    }
}
