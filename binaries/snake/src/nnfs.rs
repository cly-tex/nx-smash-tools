use core::ffi::{c_char, c_void};

#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct FileHandle(u64);

#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct WriteOptions(u32);

impl WriteOptions {
    pub const FLUSH: Self = Self(1);
}

#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct OpenMode(u32);

impl OpenMode {
    pub const READ: Self = Self(1);
    pub const WRITE: Self = Self(2);
    pub const APPEND: Self = Self(4);
    pub const READ_WRITE: Self = Self(3);
    pub const WRITE_APPEND: Self = Self(6);
}

unsafe extern "C" {
    #[link_name = "_ZN2nn2fs10CreateFileEPKcl"]
    pub fn create_file(path: *const c_char, size: i64) -> u32;

    #[link_name = "_ZN2nn2fs8OpenFileEPNS0_10FileHandleEPKci"]
    pub fn open_file(out_handle: *mut FileHandle, path: *const c_char, open_mode: OpenMode) -> u32;

    #[link_name = "_ZN2nn2fs9WriteFileENS0_10FileHandleElPKvmRKNS0_11WriteOptionE"]
    pub fn write_file(
        handle: FileHandle,
        offset: isize,
        data: *const c_void,
        size: usize,
        write_options: &WriteOptions,
    ) -> u32;

    #[link_name = "_ZN2nn2fs9CloseFileENS0_10FileHandleE"]
    pub fn close_file(handle: FileHandle);

    #[link_name = "_ZN2nn2fs11MountSdCardEPKc"]
    pub fn mount_sd_card(mount_path: *const c_char) -> u32;
}
