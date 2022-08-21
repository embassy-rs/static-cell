#![no_std]
#![doc = include_str!("../README.md")]

use core::cell::UnsafeCell;
use core::mem::MaybeUninit;

use atomic_polyfill::{AtomicBool, Ordering};

/// Statically allocated, initialized at runtime cell.
///
/// See the [crate-level docs](crate) for usage.
pub struct StaticCell<T> {
    used: AtomicBool,
    val: UnsafeCell<MaybeUninit<T>>,
}

unsafe impl<T> Send for StaticCell<T> {}
unsafe impl<T> Sync for StaticCell<T> {}

impl<T> StaticCell<T> {
    /// Create a new, uninitialized `StaticCell`.
    ///
    /// It can be initialized at runtime with [`StaticCell::init()`].
    #[inline]
    pub const fn new() -> Self {
        Self {
            used: AtomicBool::new(false),
            val: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }

    /// Initialize the `StaticCell` with a value, returning a mutable reference to it.
    ///
    /// Using this method, the compiler usually constructs `val` in the stack and then moves
    /// it into the `StaticCell`. If `T` is big, this is likely to cause stack overflows.
    /// Considering using [`StaticCell::init_with`] instead, which will construct it in-place inside the `StaticCell`.
    ///
    /// # Panics
    ///
    /// Panics if this `StaticCell` already has a value stored in it.
    #[inline]
    #[allow(clippy::mut_from_ref)]
    pub fn init(&'static self, val: T) -> &'static mut T {
        self.init_with(|| val)
    }

    /// Initialize the `StaticCell` with the closure's return value, returning a mutable reference to it.
    ///
    /// The advantage over [`StaticCell::init`] is that this method allows the closure to construct
    /// the `T` value in-place directly inside the `StaticCell`, saving stack space.
    ///
    /// # Panics
    ///
    /// Panics if this `StaticCell` already has a value stored in it.
    #[inline]
    #[allow(clippy::mut_from_ref)]
    pub fn init_with(&'static self, val: impl FnOnce() -> T) -> &'static mut T {
        if self
            .used
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            panic!("StaticCell::init() called multiple times");
        }

        let p: &mut MaybeUninit<T> = unsafe { &mut *self.val.get() };
        p.write(val())
    }
}
