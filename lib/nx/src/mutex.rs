/* A brief explanation of exclusive memory access
 *  - This is summarized from the ARM documentation here: https://developer.arm.com/documentation/102336/0101
 *
 * 1. Processors are allowed to peform operations "out of order" as long as it is invisible to the software
 *    This means that if you had some code like this:
 *    ```rs
 *    fn add(val: &mut i32, op: i32) -> i32 {
 *        let value = *val;
 *        *val = value + op;
 *        return *val;
 *    }
 *    ```
 *    Even though the writing/reading to/from memory might happen out of order in reality, the generated assembly can never observe
 *    the old contents of `value` after you've updated it
 * 2. This only applies to individual "observers", i.e. whichever CPU core is running the above code would have the above guarantee,
 *    but let's look at a slightly different example that breaks Rust's aliasing guarantees
 *    ```rs
 *    // Run on CPU Core 0
 *    fn log_values(val_a: &i32, val_b: &i32) {
 *        println!("{val_a} | {val_b}")
 *    }
 *
 *    // Run on CPU Core 1
 *    fn write_values(val_a: &mut i32, val_b: &mut i32) {
 *        *val_a = 0x1;
 *        *val_b = 0x1;
 *    }
 *    ```
 *    If we assume that `val_a` and `val_b` are both initialized to 0x0, and that both of these two code blocks are running simultaneously
 *    on different cores, then the table of possible logs are as follows:
 *    0 | 0
 *    0 | 1
 *    1 | 0
 *    1 | 1
 *    Which is strange, but shows that the memory write for val_a and val_b is not synchronized across observers
 * 3. Let's rewrite the above methods as assembly:
 *    ```arm
 *    // log_values
 *    ldr w0, [val_a]
 *    ldr w1, [val_b]
 *
 *    // write_values
 *    str #1, [val_a]
 *    str #1, [val_b]
 *    ```
 *    If we rewrite the write_values method to use the `stlr` instruction instead, we can reduce the number of rows in the above table:
 *    ```arm
 *    // write_values
 *    str  #1, [val_a]
 *    stlr #1, [val_b]
 *    ```
 *    Table:
 *    0 | 0
 *    1 | 0
 *    1 | 1
 *    Now it's impossible for core 0 to observe val_b being updated without val_a also being updated.
 * 4. Sometimes though, you will want to operate on data assuming that you are the **only** observer that is actively modifying the state.
 *    For example, consider the following situation: Two separate cores are responsible for performing an addition on the same number. Some function
 *    that looks like this:
 *    ```rs
 *    fn add_one(value: &mut i32) {
 *        *value += 1;
 *    }
 *    ```
 *
 *    You'll need to read the contents of `value`, add `1` to it, then write it back to `value`. If two separate cores are doing things, then we need
 *    to make sure that we don't have something that looks like this:
 *    a. Core0 reads value = 0
 *    b. Core1 reads value = 0
 *    c. Core1 writes value = 1
 *    d. Core1 reads value = 1
 *    e. Core1 writes value = 2
 *    f. Core0 writes value = 1
 *
 *    To do this, we introduce some "helper" methods which help manage exclusivity, and this goes down to the processor level:
 *    ```rs
 *    fn load_exclusive(value: &i32) -> i32 { /* ... */ }
 *    fn write_exclusive(ptr: &mut i32, value: i32) -> bool { /* ... */ }
 *    ```
 *
 *    In these methods, `load_exclusive` will mark the pointer `value` as us having "exclusive" access to it, and then `write_exclusive` will attempt
 *    to write to that pointer and returns whether the write was successful. The write's success depends on a number of factors,
 *    but if it was successful, then it's guaranteed that *no other observer wrote to value*. Note that having "exclusive" access marked does not prevent
 *    other observers from reading the value, or writing the value, it is basically a "two-key" system to prevent write collisions.
 *
 *    This means that in our above "bad execution" trace, at step `f`, `Core0` would fail to write.
 *    Then you can implement the `add_one` method differently:
 *    ```rs
 *    fn add_one(ptr: &mut i32) {
 *        let mut value = load_exclusive(ptr);
 *        while !store_exclusive(ptr, value + 1) {
 *            value = load_exclusive(ptr);
 *        }
 *    }
 *    ```
 *    This would guarantee that each core cannot undo the work of another core, even if it means redoing the work it already did.
 *
 *    This is useful for mutex implementations to ensure that there is no race condition on the acquisition. For example:
 *    a. Core0 sees that mutex is not locked (load-exclusive)
 *    b. Core1 sees that mutex is not locked (load-exclusive)
 *    c. Core2 sees that mutex is not locked (load-exclusive)
 *    d. Core0 stores a unique id in the mutex to lock it (store-exclusive - success)
 *    e. Core0 sleeps
 *    f. Core1 stores a unique id in the mutex to lock it (store-exclusive - fail)
 *    g. Core2 stores a unique id in the mutex to lock it (store-exclusive - fail)
 *    h. Core1 sees that the mutex is locked and no-one is waiting yet (load-exclusive)
 *    i. Core2 sees that the mutex is locked and no-one is waiting yet (load-exclusive)
 *    j. Core2 writes a flag that indicates it's waiting on the kernel (store-exclusive - success)
 *    k. Core2 requests kernel to alert it when mutex is unlocked, goes to sleep
 *    l. Core1 writes a flag that indicates it's waiting on the kernel (store-exclusive - fail)
 *    m. Core1 sees that the mutex is locked and there are waiters (load-exclusive)
 *    n. Core1 requests kernel to alert it when mutex is unlocked, goes to sleep
 *    o. Core0 wakes up
 *    p. Core0 sees that the mutex has waiters (load-exclusive)
 *    q. Core0 signals to kernel to release the lock
 *    r. Core2 wakes up, sees that it owns the mutex (load-exclusive)
 *    s. Core1 wakes up, sees that it does not own the mutex, and that there are still waiters (load-exclusive)
 *    t. Core2 sleeps
 *    u. Core1 requests kernel to alert it when mutex is unlocked, goes to sleep
 *    v. Core2 wakes up
 *    x. Core2 sees that the mutex has waiters (load-exclusive)
 *    y. Core2 signals to kernel to release the lock
 *    z. Core1 wakes up, sees that it owns the mutex (load-exclusive)
 *    aa. Core1 sleeps
 *    ab. Core1 wakes up
 *    ac. Core1 sees that the mutex has no waiters (load-exclusive)
 *    ad. Core1 stores an invalid ID to indicate the lock is free (store-exclusive - success)
 *
 *    You can see this implementation below for the mutexes. Credits to libnx for the initial mutex implementation this is based on
 *    and for prompting me to finally learn memory access boundaries
 */

use core::cell::UnsafeCell;

use crate::svc::BreakReason;

/// Loads u32 from a pointer, marking exclusive access
#[inline(always)]
fn load_exclusive(ptr: *const u32) -> u32 {
    let out: u32;
    unsafe {
        core::arch::asm!("ldaxr {:w}, {:x}", out(reg) out, in(reg) ptr, options(nostack));
    }

    out
}

/// Stores a u32 into the specified pointer with exclusive access
///
/// # Notes
/// - This can only succeed if no other observer (core) has written to the pointer since it was acquired
/// - This can fail for other reasons as well, depending on the implementation, if it fails attmept to reload the value
///   [`load_exclusive`] and try again
#[inline(always)]
fn store_exclusive(ptr: *mut u32, value: u32) -> bool {
    let result: i32;

    unsafe {
        core::arch::asm!("stlxr {:w}, {:w}, {:x}", out(reg) result, in(reg) value, in(reg) ptr, options(nostack));
    }

    result == 0
}

/// Releases a marked region from the local monitor
#[inline(always)]
fn clear_exclusive() {
    unsafe { core::arch::asm!("clrex", options(nostack)) }
}

pub struct Mutex(UnsafeCell<u32>);

impl Mutex {
    const WAITER_MASK: u32 = 1u32 << 30;

    pub const fn new() -> Self {
        // Mutex begins in unlocked state, which would be an invalid handle
        Self(UnsafeCell::new(0u32))
    }

    /// Locks the mutex
    ///
    /// - If this mutex is unlocked, this method will acquire it and not perform any kernel arbitration.
    /// - If this mutex is locked by another thread, this method will ask the kernel to arbitrate it for us and sleep
    ///   until it is acquired
    pub fn lock(&self) {
        let current_thread_handle: u32 = crate::thread::current_thread_handle();

        let mut value = load_exclusive(self.0.get());

        loop {
            // If the value is an invalid hande, then we can attempt to lock the mutex without
            // asking the kernel
            if value == 0u32 {
                // Attempt to write our thread's handle into the mutex. It is possible that this is being called
                // from separate threads but behavior should be well defined since we are using exclusive memory
                // access instructions that will fail if something else has written this
                if !store_exclusive(self.0.get(), current_thread_handle) {
                    // We were unable to acquire the mutex so retry from the start
                    value = load_exclusive(self.0.get());
                    continue;
                }
            }

            // If there are no existing waiters, then we need to store that value in our mutex
            if value & Self::WAITER_MASK == 0 {
                // Attempt to write the same value with the waiter mask into the mutex. See above for aliasing comments
                if !store_exclusive(self.0.get(), value | Self::WAITER_MASK) {
                    // Unable to store waiter mask in the mutex, retry from the start
                    value = load_exclusive(self.0.get());
                    continue;
                }
            }

            // Ask kernel to arbitrate the lock for us so our thread can go to sleep
            if crate::svc::arbitrate_lock(
                value & !Self::WAITER_MASK,
                self.0.get(),
                current_thread_handle,
            )
            .is_err()
            {
                crate::svc::break_now(BreakReason::ASSERT);
            }

            // Check if the kernel has assigned us the new owner of the lock.
            //
            // This should only *not* be true if the previous owner of the mutex unlocked it between us
            // setting the waiter mask and calling `arbitrate_lock`.
            value = load_exclusive(self.0.get());
            if value & !Self::WAITER_MASK == current_thread_handle {
                clear_exclusive();
                break;
            }
        }
    }

    /// Attempts to lock this mutex without kernel arbitration
    ///
    /// This is effectively whether or not the happy-path of [`lock`](Self::lock) is successful, without sleeping this thread or asking
    /// the kernel to arbitrate for us.
    pub fn try_lock(&self) -> bool {
        let current_thread_handle = crate::thread::current_thread_handle();

        loop {
            let value = load_exclusive(self.0.get());

            if value != 0u32 {
                break false;
            }

            if store_exclusive(self.0.get(), current_thread_handle) {
                break true;
            }
        }
    }

    /// Unlocks this mutex
    pub fn unlock(&self) {
        let current_thread_handle = crate::thread::current_thread_handle();

        let mut value = load_exclusive(self.0.get());

        loop {
            // Cold path: Other threads are waiting on this lock, so we inform the kernel to pick the next thread
            if value != current_thread_handle {
                // If this check fails, then it means `Mutex::unlock` was called from a thread that does not own the mutex
                //
                // This should be a logical error and we should consider panicking here
                #[allow(clippy::collapsible_if)]
                if value & Self::WAITER_MASK != 0 {
                    if crate::svc::arbitrate_unlock(self.0.get()).is_err() {
                        crate::svc::break_now(BreakReason::ASSERT);
                    }
                }

                break;
            }

            if store_exclusive(self.0.get(), 0u32) {
                break;
            }

            value = load_exclusive(self.0.get());
        }
    }
}

impl Default for Mutex {
    fn default() -> Self {
        Self::new()
    }
}
