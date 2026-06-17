use core::{mem::MaybeUninit, ops::Range, pin::Pin, ptr::NonNull};

use alloc::boxed::Box;

use crate::{
    mutex::{Mutex, MutexGuard},
    random::ThreadRng,
    result::NxResult,
    svc::BreakReason,
};

pub enum AllocationType {
    Alias,
    Aslr,
    Stack,
}

struct VirtualReservationNode {
    prev: Option<NonNull<VirtualReservationNode>>,
    next: Option<NonNull<VirtualReservationNode>>,
}

struct VirtualReservation {
    node: VirtualReservationNode,
    start: u64,
    user_start: u64,
    page_size: u64,
    user_pointer: u64,
    size: u64,
}

pub const PAGE_GRANULARITY: usize = 0x1000;

pub struct VirtualReservationHandle(NonNull<VirtualReservation>);

impl VirtualReservationHandle {
    fn inner(&self) -> &VirtualReservation {
        // SAFETY: We own this pointer so a non-exclusive reference to self is good enough
        unsafe { self.0.as_ref() }
    }

    fn inner_mut(&mut self) -> &mut VirtualReservation {
        // SAFETY: We own this pointer so a non-exclusive reference to self is good enough
        unsafe { self.0.as_mut() }
    }

    /// Returns the start of the allocated memory region, this will always be aligned to [`PAGE_GRANULARITY`]
    pub fn start_of_region(&self) -> *mut u8 {
        self.inner().start as *mut u8
    }

    /// Returns the user pointer inside of the allocated memory region
    pub fn as_ptr(&self) -> *mut u8 {
        self.inner().user_pointer as *mut u8
    }

    /// Returns the size of the allocated region. This will always be a multiple of [`PAGE_GRANULARITY`]
    pub fn region_size(&self) -> u64 {
        self.inner().page_size
    }

    /// Returns the size of the user mapped region
    pub fn size(&self) -> u64 {
        self.inner().size
    }
}

unsafe impl Send for VirtualReservationHandle {}
unsafe impl Sync for VirtualReservationHandle {}

pub struct VirtualMemory {
    alias_memory: Range<u64>,
    aslr_memory: Range<u64>,
    heap_memory: Range<u64>,
    stack_memory: Range<u64>,
    root: VirtualReservationNode,
    _marker: core::marker::PhantomPinned,
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
            _marker: core::marker::PhantomPinned,
        })
    }

    pub fn release_allocation(
        self: &mut Pin<&mut Self>,
        handle: VirtualReservationHandle,
    ) -> NxResult<()> {
        let node = &handle.inner().node;

        unsafe {
            crate::svc::unmap_memory(
                handle.inner().user_start,
                handle.inner().start,
                handle.inner().page_size,
            )?;
        }

        if let Some(mut prev) = node.prev {
            unsafe { prev.as_mut().next = node.next };
        }

        if let Some(mut next) = node.next {
            unsafe { next.as_mut().prev = node.prev };
        }

        Ok(())
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
    /// ## Note about memory mapping
    /// Memory must be mapped in regions both aligned to and with a size a multiple of [the page granularity](PAGE_GRANULARITY).
    /// A region will be mapped that contains all pages required to access `user_pointer..user_pointer.add(size)`, and the returned
    /// reservation will point to the corresponding location in the mapped memory pages. For example, if `user_pointer % PAGE_GRANULARITY == 0x130`
    /// then `handle.addr() % PAGE_GRANULARITY == 0x130`, even if the start of the mapped range is different.
    ///
    /// # Safety
    /// - `user_pointer` must be a valid pointer for the entire duration of the virtual reservation's usage
    /// - Even though it is checked by the kernel and will return an error, it should be treated as UB to call
    ///   this method with a `user_pointer` that points to memory not valid for `size` bytes
    pub unsafe fn reserve_and_map(
        self: &mut Pin<&mut Self>,
        ty: AllocationType,
        user_pointer: *mut u8,
        size: usize,
        guard_size: usize,
    ) -> NxResult<VirtualReservationHandle> {
        let this = unsafe { self.as_mut().get_unchecked_mut() };

        let user_page_offset = user_pointer.align_offset(PAGE_GRANULARITY);
        let unaligned_size = size;
        let size = size + user_page_offset;
        let size = (size + PAGE_GRANULARITY - 1) & !(PAGE_GRANULARITY - 1);
        let guard_size = (guard_size + PAGE_GRANULARITY - 1) & !(PAGE_GRANULARITY - 1);
        let allocation_range = match ty {
            AllocationType::Alias => &this.alias_memory,
            AllocationType::Aslr => &this.aslr_memory,
            AllocationType::Stack => &this.stack_memory,
        };

        let region_size = allocation_range.end - allocation_range.start;
        let max_page_offset = region_size >> PAGE_GRANULARITY.trailing_zeros();

        let alloc_start = 'alloc_loop: loop {
            let page_offset = crate::random::get_u64(ThreadRng) % (max_page_offset + 1);

            let alloc_start =
                allocation_range.start + (page_offset << PAGE_GRANULARITY.trailing_zeros());
            let alloc_end = alloc_start + (size + guard_size) as u64;

            let mut node = this.root.next.map(|ptr| ptr.cast::<VirtualReservation>());
            while let Some(ptr) = node {
                let ptr = unsafe { &*ptr.as_ptr() };
                if (ptr.start <= alloc_start && alloc_start <= ptr.start + ptr.size)
                    || (ptr.start <= alloc_end && alloc_end <= ptr.start + ptr.size)
                {
                    continue 'alloc_loop;
                }

                node = ptr.node.next.map(|ptr| ptr.cast::<VirtualReservation>());
            }

            break alloc_start;
        };

        let user_start = unsafe { user_pointer.sub(user_page_offset) as u64 };

        crate::svc::map_memory(alloc_start, user_start, size as u64)?;

        let reservation = Box::leak(Box::new(VirtualReservation {
            node: VirtualReservationNode {
                prev: Some(NonNull::from_mut(&mut this.root)),
                next: this.root.next,
            },
            start: alloc_start,
            size: unaligned_size as u64,
            user_pointer: alloc_start + user_page_offset as u64,
            user_start,
            page_size: (size + guard_size) as u64,
        }));

        if let Some(mut prev_first) = this.root.next {
            unsafe { prev_first.as_mut() }.prev = Some(NonNull::from_mut(&mut reservation.node));
        }

        this.root.next = Some(NonNull::from_mut(&mut reservation.node));

        Ok(VirtualReservationHandle(NonNull::from_mut(reservation)))
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

pub fn with_virtmem<R>(f: impl FnOnce(Pin<&mut VirtualMemory>) -> R) -> R {
    let mut virtmem = virtmem();
    // SAFETY: Our virtmem is a static object so it will not ever get dropped,
    // we also won't be moving it. The user does not have control over the raw memory
    f(unsafe { Pin::new_unchecked(&mut *virtmem) })
}

/// Maps an address with the specified allocation type
///
/// # Safety
/// The caller must ensure that address is in user-mapped memory space and is valid for read/writes between
/// `address` and `address.add(size)`
pub unsafe fn map(
    address: *mut u8,
    size: usize,
    ty: AllocationType,
) -> NxResult<VirtualReservationHandle> {
    // SAFETY: User upholds safety restrictions
    with_virtmem(|mut virtmem| unsafe { virtmem.reserve_and_map(ty, address, size, 0x4000) })
}
