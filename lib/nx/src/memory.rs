use crate::result::NxResultExt;

mod critical_section;

// Heap must be allocated in multiples of 2 MB
const HEAP_GRANULARITY: u64 = 2 * 1024 * 1024;

#[global_allocator]
static HEAP: embedded_alloc::LlffHeap = embedded_alloc::LlffHeap::empty();

/// Initializes the memory allocator and heap region
///
/// # Safety
/// This method must only be called once during the lifecycle of the process and must be called
/// before any memory allocations are attempted.
pub(crate) unsafe fn init() {
    let total_size = crate::svc::info::total_memory_size().assert();
    let used_size = crate::svc::info::used_memory_size().assert();

    let mut size = 0u64;

    // There should be at least a 2MB range we can allocate for the heap
    if total_size > used_size {
        // Libnx subtracts an additional HEAP_GRANULARITY, I'm not sure why? I'm not going to include it here but it's worth noting
        size = (total_size - used_size) & !(HEAP_GRANULARITY - 1);
    }

    if size == 0 {
        // Possibly problematic because that means there's no heap left??
        // We will just try to request a single granularitya unit of the heap
        size = HEAP_GRANULARITY
    }

    // SAFETY: Caller upholds invariants
    let heap_base = unsafe { crate::svc::set_heap_size(size).assert() };

    // SAFETY: As long as the kernel impl is correct then both invariants should hold
    unsafe { HEAP.init(heap_base as usize, size as usize) };
}
