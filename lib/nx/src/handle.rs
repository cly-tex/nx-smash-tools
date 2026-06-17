#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct InvalidHandle;

impl InvalidHandle {
    pub const fn into_inner(self) -> u32 {
        0u32
    }
}

#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct ProcessHandle(u32);

impl ProcessHandle {
    pub const THIS_PROCESS: Self = Self(0xFFFF8001);

    /// # Safety
    /// The caller must ensure that the handle is actually a process handle
    pub unsafe fn from_raw(raw_handle: u32) -> Self {
        Self(raw_handle)
    }

    pub const fn into_inner(self) -> u32 {
        self.0
    }
}
