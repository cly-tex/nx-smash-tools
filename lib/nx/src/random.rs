//! Manages global and per-thread RNG
//!
//! # Implementation
//! The implementation of RNG utilizes the [ChaCha algorithm](https://en.wikipedia.org/wiki/ChaCha20-Poly1305)
//! for generating random numbers.
//!
//! The random seed is taken from the operating environment. Thread RNG uses the same raw seed as global RNG
//! but also incorporates the thread handle to make it distinguished from global RNG.

use core::mem::MaybeUninit;

use alloc::boxed::Box;
use chacha::{ChaCha, KeyStream};

use crate::mutex::{Mutex, MutexGuard};

static GLOBAL_RNG: Mutex<MaybeUninit<ChaCha>> = Mutex::new(MaybeUninit::uninit());

fn create_chacha_with_thread_handle(handle: u32) -> ChaCha {
    let handle_xor = handle as u64 | (handle as u64) << 32;

    let seed = match crate::svc::info::random_entropy() {
        Ok(seed) => seed,
        Err(_) => crate::svc::assert_fail(),
    };

    let seed = seed.map(|value| value ^ handle_xor);

    let seed = unsafe { &*(&raw const seed).cast::<[u8; 32]>() };

    ChaCha::new_chacha20(seed, &[0u8; 0x8])
}

fn global_rng() -> MutexGuard<'static, ChaCha> {
    // SAFETY: We can assume that the global RNG has been initialized during process init
    unsafe { GLOBAL_RNG.lock_assume_init() }
}

/// Initializes global thread RNG
///
/// # Safety
/// This method must only be called once, and should be called during process init
pub(crate) unsafe fn init_global_rng() {
    GLOBAL_RNG.lock().write(create_chacha_with_thread_handle(0));
}

/// Initializes thread RNG
///
/// Calling any RNG method with [`ThreadRng`] assumes that this method has been called
/// on the calling thread.
///
/// # Safety
/// This method must only be called once-per-thread.
/// - For the main thread, this must be sequenced after heap memory is initialized
/// - For non-main threads, this should be called during thread env setup
pub(crate) unsafe fn init_thread_rng() {
    let chacha = Box::new(create_chacha_with_thread_handle(
        crate::thread::current_thread_handle(),
    ));

    crate::thread::with_tls(|tls| {
        tls.variables_mut().random.write(chacha);
    });
}

#[doc(hidden)]
mod __sealed {
    pub trait Sealed {}
}

pub trait NxRng: __sealed::Sealed {
    /// Fill bytes with random data
    ///
    /// # Safety
    /// The caller must ensure that `bytes` is valid for read/writes for up to `len` bytes
    unsafe fn get(bytes: *mut u8, len: usize);
}

pub struct GlobalRng;
pub struct ThreadRng;

impl __sealed::Sealed for GlobalRng {}
impl __sealed::Sealed for ThreadRng {}

impl NxRng for GlobalRng {
    unsafe fn get(bytes: *mut u8, len: usize) {
        let mut rng = global_rng();

        unsafe {
            core::ptr::write_bytes(bytes, 0u8, len);
            let _ = rng.xor_read(core::slice::from_raw_parts_mut(bytes, len));
        }
    }
}

impl NxRng for ThreadRng {
    unsafe fn get(bytes: *mut u8, len: usize) {
        crate::thread::with_tls(|tls| {
            // SAFETY: We establish that we assume RNG to be initialized by the time this method is called
            let rng = unsafe { tls.variables_mut().random.assume_init_mut() };

            unsafe {
                core::ptr::write_bytes(bytes, 0u8, len);
                let _ = rng.xor_read(core::slice::from_raw_parts_mut(bytes, len));
            }
        })
    }
}

/// # Safety
/// The caller must ensure that `bytes` is valid for reading/writing up to `len` bytes
pub unsafe fn fill<T: NxRng>(_rng: T, bytes: *mut u8, len: usize) {
    unsafe { T::get(bytes, len) }
}

macro_rules! decl_get {
    ($($name:ident -> $val_ty:ty;)*) => {
        $(
            pub fn $name(rng: impl NxRng) -> $val_ty {
                let mut out: MaybeUninit<$val_ty> = MaybeUninit::uninit();
                unsafe { fill(rng, out.as_mut_ptr().cast::<u8>(), size_of::<$val_ty>()) };
                unsafe { out.assume_init() }
            }
        )*
    };
}

decl_get! {
    get_u8 -> u8;
    get_i8 -> i8;
    get_u16 -> u16;
    get_i16 -> i16;
    get_u32 -> u32;
    get_i32 -> i32;
    get_u64 -> u64;
    get_i64 -> i64;
    get_u128 -> u128;
    get_i128 -> i128;
}
