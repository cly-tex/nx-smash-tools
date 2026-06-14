use core::{mem::MaybeUninit, ops::Range};

use crate::{handle::ProcessHandle, result::NxResult};

use super::svcGetInfo;

#[repr(u32)]
enum InfoType {
    AliasRegionAddr = 2,
    AliasRegionSize = 3,
    HeapRegionAddr = 4,
    HeapRegionSize = 5,
    TotalMemorySize = 6,
    UsedMemorySize = 7,
    AslrRegionAddr = 12,
    AslrRegionSize = 13,
    StackRegionAddr = 14,
    StackRegionSize = 15,
    AliasRegionExtraSize = 18,
}

macro_rules! get_u64 {
    ($info_ty:ident, $handle:expr) => {{
        let mut out = MaybeUninit::uninit();
        unsafe {
            svcGetInfo(
                out.as_mut_ptr(),
                InfoType::$info_ty as u32,
                $handle.into_inner(),
                0,
            )
        }
        .then(|| unsafe { out.assume_init() })
    }};
}

#[inline(always)]
pub fn alias_region() -> NxResult<Range<u64>> {
    let address = get_u64!(AliasRegionAddr, ProcessHandle::THIS_PROCESS)?;
    let mut size = get_u64!(AliasRegionSize, ProcessHandle::THIS_PROCESS)?;
    if let Ok(extra_size) = get_u64!(AliasRegionExtraSize, ProcessHandle::THIS_PROCESS) {
        size -= extra_size;
    }

    Ok(address..address + size)
}

#[inline(always)]
pub fn heap_region() -> NxResult<Range<u64>> {
    let address = get_u64!(HeapRegionAddr, ProcessHandle::THIS_PROCESS)?;
    let size = get_u64!(HeapRegionSize, ProcessHandle::THIS_PROCESS)?;

    Ok(address..address + size)
}

#[inline(always)]
pub fn aslr_region() -> NxResult<Range<u64>> {
    let address = get_u64!(AslrRegionAddr, ProcessHandle::THIS_PROCESS)?;
    let size = get_u64!(AslrRegionSize, ProcessHandle::THIS_PROCESS)?;

    Ok(address..address + size)
}

#[inline(always)]
pub fn stack_region() -> NxResult<Range<u64>> {
    let address = get_u64!(StackRegionAddr, ProcessHandle::THIS_PROCESS)?;
    let size = get_u64!(StackRegionSize, ProcessHandle::THIS_PROCESS)?;

    Ok(address..address + size)
}

#[inline(always)]
pub fn total_memory_size() -> NxResult<u64> {
    get_u64!(TotalMemorySize, ProcessHandle::THIS_PROCESS)
}

#[inline(always)]
pub fn used_memory_size() -> NxResult<u64> {
    get_u64!(UsedMemorySize, ProcessHandle::THIS_PROCESS)
}
