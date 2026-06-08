#![no_std]

use core::mem::MaybeUninit;

use crate::nnfs::{FileHandle, OpenMode, WriteOptions};

mod nnfs;

#[unsafe(export_name = "snake_module_object")]
static mut SNAKE_MODULE_OBJECT: MaybeUninit<rtld::ModuleObject> = MaybeUninit::uninit();

core::arch::global_asm!(include_str!("crt0.s"));

unsafe extern "C" {
    #[link_name = "_ZN2nn2oe15ExitApplicationEv"]
    fn exit_app() -> !;
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    unsafe { exit_app() };
}

#[unsafe(no_mangle)]
pub extern "C" fn main() {
    unsafe {
        assert_eq!(nnfs::mount_sd_card(c"sd".as_ptr()), 0);
        // assert_eq!(nnfs::create_file(c"sd:/snake_log_test.log".as_ptr(), 0), 0);

        let mut file = MaybeUninit::uninit();
        assert_eq!(
            nnfs::open_file(
                file.as_mut_ptr(),
                c"sd:/snake_log_test.log".as_ptr(),
                OpenMode::WRITE_APPEND
            ),
            0
        );
        let file = file.assume_init();

        assert_eq!(
            nnfs::write_file(
                file,
                0,
                b"Hello from snake logger!\n".as_ptr().cast(),
                25,
                &WriteOptions::FLUSH
            ),
            0
        );

        nnfs::close_file(file);
    }
}
