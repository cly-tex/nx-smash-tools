use core::num::NonZeroU32;

#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct InvalidHandle;

impl InvalidHandle {
    pub const fn into_inner(self) -> u32 {
        0u32
    }
}

#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct ThreadHandle(NonZeroU32);

impl ThreadHandle {
    pub const THIS_THREAD: Self = Self(const { NonZeroU32::new(0xFFFF8000).unwrap() });

    /// # Safety
    /// Caller must ensure that the handle being passed in is a thread handle and is non-zero
    pub(crate) const unsafe fn new(inner: u32) -> Self {
        // SAFETY: Caller upholds safety requirements
        Self(unsafe { NonZeroU32::new_unchecked(inner) })
    }

    pub const fn into_inner(self) -> u32 {
        self.0.get()
    }
}

#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct ProcessHandle(NonZeroU32);

impl ProcessHandle {
    pub const THIS_PROCESS: Self = Self(const { NonZeroU32::new(0xFFFF8001).unwrap() });

    /// # Safety
    /// Caller must ensure that the handle being passed in is a process handle and is non-zero
    pub(crate) unsafe fn new(raw_handle: u32) -> Self {
        // SAFETY: Caller upholds safety requirements
        Self(unsafe { NonZeroU32::new_unchecked(raw_handle) })
    }

    pub const fn into_inner(self) -> u32 {
        self.0.get()
    }
}
