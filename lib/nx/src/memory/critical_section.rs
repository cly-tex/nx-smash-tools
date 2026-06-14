struct CriticalSection;
critical_section::set_impl!(CriticalSection);

static LOCK: crate::mutex::RawMutex = crate::mutex::RawMutex::new();

const LOCK_STATE_ACQUIRED: u8 = 0x0;
const LOCK_STATE_RECURSIVE: u8 = 0x1;

unsafe impl critical_section::Impl for CriticalSection {
    unsafe fn acquire() -> u8 {
        if LOCK.locked_by_this_thread() {
            LOCK_STATE_RECURSIVE
        } else {
            LOCK.lock();
            LOCK_STATE_ACQUIRED
        }
    }

    unsafe fn release(restore_state: u8) {
        match restore_state {
            LOCK_STATE_ACQUIRED => LOCK.unlock(),
            LOCK_STATE_RECURSIVE => {}
            _ => unsafe { core::hint::unreachable_unchecked() },
        }
    }
}
