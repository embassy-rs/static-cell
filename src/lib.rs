#![no_std]
#![doc = include_str!("../README.md")]

use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::ptr;

use atomic_polyfill::{AtomicU8, Ordering};

/// Statically allocated, initialized at runtime cell.
///
/// See the [crate-level docs](crate) for usage.
pub struct StaticCell<T> {
    state: AtomicU8,
    val: UnsafeCell<MaybeUninit<T>>,
}

unsafe impl<T> Send for StaticCell<T> {}
unsafe impl<T> Sync for StaticCell<T> {}

const STATE_UNINIT: u8 = 0;
const STATE_INIT: u8 = 1;
const STATE_TAKEN: u8 = 2;

impl<T> StaticCell<T> {
    /// Create a new, uninitialized `StaticCell`.
    ///
    /// It can be initialized at runtime with [`StaticCell::init`] or [`StaticCell::init_with`].
    #[inline]
    pub const fn new() -> Self {
        Self {
            state: AtomicU8::new(STATE_UNINIT),
            val: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }

    /// Create a new `StaticCell` initialized with the given value.
    ///
    /// A mutable reference to the value can be taken at runtime using [`StaticCell::take`].
    #[inline]
    pub const fn new_with_value(value: T) -> Self {
        Self {
            state: AtomicU8::new(STATE_INIT),
            val: UnsafeCell::new(MaybeUninit::new(value)),
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
            .state
            .compare_exchange(
                STATE_UNINIT,
                STATE_TAKEN,
                Ordering::Acquire,
                Ordering::Relaxed,
            )
            .is_err()
        {
            panic!("`StaticCell` cannot be initialized twice");
        }

        let p: &mut MaybeUninit<T> = unsafe { &mut *self.val.get() };
        p.write(val())
    }

    /// Take the mutable reference to the contained value.
    ///
    /// If the mutable reference was taken previously, returns `None`.
    ///
    /// # Panics
    ///
    /// Panics if this `StaticCell` is uninitialized.
    #[inline]
    pub fn take(&'static self) -> Option<&'static mut T> {
        if let Err(state) = self.state.compare_exchange(
            STATE_INIT,
            STATE_TAKEN,
            Ordering::Acquire,
            Ordering::Relaxed,
        ) {
            match state {
                STATE_UNINIT => panic!("`StaticCell` is not initialized"),
                STATE_TAKEN => return None,
                _ => unreachable!(),
            }
        }

        // SAFETY: We just asserted that the value is initialized and not taken.
        Some(unsafe { (&mut *self.val.get()).assume_init_mut() })
    }

    /// Return the mutable reference so it can be taken again.
    ///
    /// # Panics
    ///
    /// This panics if the reference does not point to the value of
    /// this `StaticCell` or if it was not taken before.
    #[inline]
    pub fn restore(&'static self, value: &'static mut T) {
        if ptr::addr_of!(*value).cast() != ptr::addr_of!(self.val) {
            panic!("value does not belong to this `StaticCell`");
        }

        if let Err(state) = self.state.compare_exchange(
            STATE_TAKEN,
            STATE_INIT,
            Ordering::Acquire,
            Ordering::Relaxed,
        ) {
            match state {
                STATE_UNINIT => panic!("`StaticCell` is not initialized"),
                STATE_INIT => panic!("cannot restore `StaticCell` value which was not taken"),
                _ => unreachable!(),
            }
        }
    }
}
