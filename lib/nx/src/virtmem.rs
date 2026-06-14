use core::{mem::MaybeUninit, ops::Range, pin::Pin, ptr::NonNull};

use crate::{
    mutex::{Mutex, MutexGuard},
    result::NxResult,
    svc::BreakReason,
};

struct VirtualReservationNode {
    prev: Option<NonNull<VirtualReservation>>,
    next: Option<NonNull<VirtualReservation>>,
}

struct VirtualReservation {
    node: VirtualReservationNode,
    start: u64,
    size: u64,
}

#[derive(Copy, Clone)]
pub struct VirtualReservationHandle(NonNull<VirtualReservation>);

unsafe impl Send for VirtualReservationHandle {}
unsafe impl Sync for VirtualReservationHandle {}

struct VirtualMemory {
    alias_memory: Range<u64>,
    aslr_memory: Range<u64>,
    heap_memory: Range<u64>,
    stack_memory: Range<u64>,
    root: VirtualReservationNode,
    marker: core::marker::PhantomPinned,
}

impl VirtualMemory {
    fn new() -> NxResult<Self> {
        let alias_memory = crate::svc::info::alias_region()?;
        let aslr_memory = crate::svc::info::aslr_region()?;
        let heap_memory = crate::svc::info::heap_region()?;
        let stack_memory = crate::svc::info::stack_region()?;

        Ok(Self {
            alias_memory,
            aslr_memory,
            heap_memory,
            stack_memory,
            root: VirtualReservationNode {
                prev: None,
                next: None,
            },
            marker: core::marker::PhantomPinned,
        })
    }

    /// Reserves a region of memory with the specified type and maps it to the provided pointer
    ///
    /// # Arguments
    /// - `ty` - The [memory type](AllocationType) to allocate from
    /// - `user_pointer` - The start of the user address range to map
    /// - `size` - The size of the range to map
    /// - `guard_size` - The amount of memory to allocate around the allocated pages to protect against corruption
    ///
    /// ## Note about guard pages
    /// Guard pages help prevent accidentally memory overflows (particularly on the stack). If a region
    /// of memory has an overflow with unmapped guard pages around it, the application/process will crash.
    /// If the pages around it are mapped just to other memory, it won't crash and will instead have
    /// undefined behavior.
    ///
    /// # Safety
    /// - `user_pointer` must be a valid pointer for the entire duration of the virtual reservation's usage
    /// - Even though it is checked by the kernel and will return an error, it should be treated as UB to call
    ///   this method with a `user_pointer` that points to memory not valid for `size` bytes
    pub unsafe fn reserve_and_map(
        this: Pin<&mut Self>,
        ty: AllocationType,
        user_pointer: *mut u8,
        size: usize,
        guard_size: usize,
    ) -> NxResult<VirtualReservationHandle> {
    }
}

static VIRTUAL_MEMORY: Mutex<MaybeUninit<VirtualMemory>> = Mutex::new(MaybeUninit::uninit());

#[inline(always)]
fn virtmem() -> MutexGuard<'static, VirtualMemory> {
    // SAFETY: We are allowed to assume that init() has been called
    unsafe { VIRTUAL_MEMORY.lock_assume_init() }
}

pub(crate) unsafe fn init() {
    match VirtualMemory::new() {
        Ok(mem) => {
            let mut uninit = VIRTUAL_MEMORY.lock();
            uninit.write(mem);
        }
        Err(_e) => {
            // Failing the info queries should be impossible
            crate::svc::break_now(BreakReason::ASSERT);
        }
    }
}

pub enum AllocationType {
    Alias,
    Aslr,
    Stack,
}

pub struct VirtualAllocation {
    pub address: *mut u8,
    pub handle: usize,
    pub ty: AllocationType,
}
