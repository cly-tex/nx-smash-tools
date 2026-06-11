use core::{mem::MaybeUninit, ops::Range};

use crate::{handle::ProcessHandle, result::NxResult};

use super::svcGetInfo;

#[repr(u32)]
enum InfoType {
    AliasRegionAddr = 2,
    AliasRegionSize = 3,
    HeapRegionAddr = 4,
    HeapRegionSize = 5,
    AslrRegionAddr = 12,
    AslrRegionSize = 13,
    StackRegionAddr = 14,
    StackRegionSize = 15,
}

#[inline(always)]
pub fn alias_region() -> NxResult<Range<u64>> {
    let mut address = MaybeUninit::uninit();
    let mut size = MaybeUninit::uninit();

    let address = unsafe {
        svcGetInfo(
            address.as_mut_ptr(),
            InfoType::AliasRegionAddr as u32,
            ProcessHandle::THIS_PROCESS.into_inner(),
            0,
        )
    }
    .then(|| unsafe { address.assume_init() })?;

    let size = unsafe {
        svcGetInfo(
            size.as_mut_ptr(),
            InfoType::AliasRegionSize as u32,
            ProcessHandle::THIS_PROCESS.into_inner(),
            0,
        )
    }
    .then(|| unsafe { size.assume_init() })?;

    Ok(address..address + size)
}

#[inline(always)]
pub fn heap_region() -> NxResult<Range<u64>> {
    let mut address = MaybeUninit::uninit();
    let mut size = MaybeUninit::uninit();

    let address = unsafe {
        svcGetInfo(
            address.as_mut_ptr(),
            InfoType::HeapRegionAddr as u32,
            ProcessHandle::THIS_PROCESS.into_inner(),
            0,
        )
    }
    .then(|| unsafe { address.assume_init() })?;

    let size = unsafe {
        svcGetInfo(
            size.as_mut_ptr(),
            InfoType::HeapRegionAddr as u32,
            ProcessHandle::THIS_PROCESS.into_inner(),
            0,
        )
    }
    .then(|| unsafe { size.assume_init() })?;

    Ok(address..address + size)
}

#[inline(always)]
pub fn aslr_region() -> NxResult<Range<u64>> {
    let mut address = MaybeUninit::uninit();
    let mut size = MaybeUninit::uninit();

    let address = unsafe {
        svcGetInfo(
            address.as_mut_ptr(),
            InfoType::AslrRegionAddr as u32,
            ProcessHandle::THIS_PROCESS.into_inner(),
            0,
        )
    }
    .then(|| unsafe { address.assume_init() })?;

    let size = unsafe {
        svcGetInfo(
            size.as_mut_ptr(),
            InfoType::AslrRegionAddr as u32,
            ProcessHandle::THIS_PROCESS.into_inner(),
            0,
        )
    }
    .then(|| unsafe { size.assume_init() })?;

    Ok(address..address + size)
}

#[inline(always)]
pub fn stack_region() -> NxResult<Range<u64>> {
    let mut address = MaybeUninit::uninit();
    let mut size = MaybeUninit::uninit();

    let address = unsafe {
        svcGetInfo(
            address.as_mut_ptr(),
            InfoType::StackRegionAddr as u32,
            ProcessHandle::THIS_PROCESS.into_inner(),
            0,
        )
    }
    .then(|| unsafe { address.assume_init() })?;

    let size = unsafe {
        svcGetInfo(
            size.as_mut_ptr(),
            InfoType::StackRegionAddr as u32,
            ProcessHandle::THIS_PROCESS.into_inner(),
            0,
        )
    }
    .then(|| unsafe { size.assume_init() })?;

    Ok(address..address + size)
}
