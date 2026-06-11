use core::{mem::MaybeUninit, ops::Range};

use crate::{
    mutex::{Mutex, MutexGuard},
    result::NxResult,
    svc::BreakReason,
};

struct VirtualMemory {
    alias_memory: Range<u64>,
    aslr_memory: Range<u64>,
    heap_memory: Range<u64>,
    stack_memory: Range<u64>,
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
        })
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
