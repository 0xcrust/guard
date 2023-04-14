use crate::sem::{SemGuard, SemVar};
use std::cell::UnsafeCell;

/// A Semaphore-based Mutex.
pub struct Mutex<T> {
    inner: SemVar<UnsafeCell<T>>,
}

/// SAFETY: It's safe to share across threads since 
/// single access is enforced.
unsafe impl<T> Sync for Mutex<T> where T: Send {}

/// A guard that represents exclusive access to the guarded value.
pub struct MutexGuard<'a, T>(SemGuard<'a, UnsafeCell<T>>);

impl<T> Mutex<T> {
    /// Create a new Mutex guarding value T.
    pub fn new(value: T) -> Self {
        Self {
            inner: SemVar::new(1, UnsafeCell::new(value)),
        }
    }

    /// Try to gain access to the protected value. Returns
    /// a [SemGuard].
    pub fn lock(&self) -> MutexGuard<T> {
        let guard = self.inner.access();
        MutexGuard(guard)
    }
}

use std::ops::{Deref, DerefMut};

impl<T> Deref for MutexGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.0.deref().get() }
    }
}

impl<T> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.0.deref().get() }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn mutex_test_single_thread() {
        let m = Mutex::new(0);
        std::hint::black_box(&m);
        for _ in 0..100 {
            *m.lock() += 1;
        }
        assert_eq!(*m.lock(), 100);
    }

    #[test]
    fn mutex_test_multi_threads() {
        let m = Mutex::new(0);
        std::hint::black_box(&m);
        std::thread::scope(|s| {
            for _ in 0..4 {
                s.spawn(|| {
                    for _ in 0..100 {
                        *m.lock() += 1;
                    }
                });
            }
        });
        assert_eq!(*m.lock(), 400);
    }
}
